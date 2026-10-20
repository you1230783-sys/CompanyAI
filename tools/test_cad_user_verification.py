"""測試實測工具的版本判定；純資料檢查，不啟動CAD、不冒充原生相容性。"""
import unittest
import hashlib
import json
from pathlib import Path
import tempfile

from verify_cad_user_case import summarize_artifacts, version_review
from run_cad_user_cases import has_partial_failure, validate_completed_case, validate_resume_manifest


class VersionReviewTests(unittest.TestCase):
    def test_successful_file_does_not_hide_another_partial_write(self):
        self.assertTrue(has_partial_failure([
            {'status': 'DATA_VERIFIED_VISUAL_PENDING', 'saved': True},
            {'status': 'FAILED', 'drawing_started': True, 'saved': False}]))
        self.assertFalse(has_partial_failure([
            {'status': 'DATA_VERIFIED_VISUAL_PENDING', 'saved': True},
            {'status': 'FAILED', 'drawing_started': False, 'file_exists': False}]))

    def completed_fixture(self, root, visual=True):
        """合成檔只測續跑證據／雜湊，不能當成真實DWG驗證。"""
        folder = Path(root)
        output = folder/'project/_AI_Output/case'
        output.mkdir(parents=True)
        drawing = output/'synthetic.dwg'
        drawing.write_bytes(b'synthetic fixture, not a native DWG')
        item = {'case': 'door', 'year': '2008', 'status': 'VISUAL_ISSUES'}
        files = {
            folder/'case-result.json': item,
            output/'cad-report.json': {'status': 'DATA_VERIFIED_VISUAL_PENDING'},
            folder/'independent-verification.json': {
                'dwg': str(drawing), 'sha256': hashlib.sha256(drawing.read_bytes()).hexdigest()}}
        if visual:
            files[folder/'visual-review.json'] = {'status': 'ISSUES'}
        for path, value in files.items():
            path.write_text(json.dumps(value), encoding='utf-8')
        return folder, item, drawing

    def test_completed_case_can_resume_after_review_and_unchanged_hash(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[1]/'.build') as root:
            folder, item, _ = self.completed_fixture(root)
            validate_completed_case(folder, item, True)

    def test_changed_drawing_cannot_reuse_previous_review(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[1]/'.build') as root:
            folder, item, drawing = self.completed_fixture(root)
            drawing.write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, '後來變更'):
                validate_completed_case(folder, item, True)

    def test_missing_visual_review_cannot_be_skipped_during_resume(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[1]/'.build') as root:
            folder, item, _ = self.completed_fixture(root, visual=False)
            with self.assertRaisesRegex(ValueError, '畫面覆核'):
                validate_completed_case(folder, item, True)

    def test_each_delivered_file_requires_its_own_review_data(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[1]/'.build') as root:
            folder, item, _ = self.completed_fixture(root)
            extra = folder/'project/_AI_Output/another'
            extra.mkdir()
            (extra/'cad-report.json').write_text(json.dumps({'status': 'DATA_VERIFIED_VISUAL_PENDING'}), encoding='utf-8')
            with self.assertRaisesRegex(ValueError, '每份交付檔'):
                validate_completed_case(folder, item, True)

    def test_resume_requires_identical_candidate_and_cases(self):
        configuration = dict(executable_sha256='exe', runtime_sha256='runtime', cases=['door'], versions=['2008'])
        validate_resume_manifest(dict(configuration, started='earlier'), configuration)
        for key in configuration:
            changed = dict(configuration, **{key: 'changed'})
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, key):
                validate_resume_manifest(changed, configuration)

    def test_legacy_manifest_without_candidate_hashes_cannot_resume_implicitly(self):
        with self.assertRaises(ValueError):
            validate_resume_manifest({'cases': []}, {'cases': [], 'executable_sha256': 'exe'})

    def test_all_artifacts_must_pass_data_checks(self):
        results = [{'status': 'DATA_PASS_VISUAL_PENDING', 'version_mismatch': False, 'failures': []}] * 2
        summary = summarize_artifacts(results)
        self.assertEqual(summary['status'], 'DATA_PASS_VISUAL_PENDING')
        self.assertEqual(summary['artifact_count'], 2)

    def test_one_wrong_version_cannot_be_hidden_by_another_success(self):
        summary = summarize_artifacts([
            {'status': 'DATA_PASS_VISUAL_PENDING', 'failures': []},
            {'status': 'WRONG_CAD_VERSION', 'version_mismatch': True, 'failures': ['錯版']}])
        self.assertEqual(summary['status'], 'WRONG_CAD_VERSION')
        self.assertEqual(summary['failures'], ['錯版'])

    def test_one_incomplete_artifact_does_not_become_a_pass(self):
        summary = summarize_artifacts([
            {'status': 'DATA_PASS_VISUAL_PENDING', 'failures': []},
            {'status': 'DATA_ISSUES', 'failures': ['缺少尺寸']}])
        self.assertEqual(summary['status'], 'DATA_ISSUES')

    def test_empty_artifact_list_has_no_success_evidence(self):
        with self.assertRaises(ValueError):
            summarize_artifacts([])

    def test_matching_progid_is_case_insensitive(self):
        actual, mismatch = version_review({'selected': {'progid': 'Icad.Application.1'}}, 'icad.application.1')
        self.assertEqual(actual, 'Icad.Application.1')
        self.assertIsNone(mismatch)

    def test_wrong_version_is_reviewed_in_actual_application_and_never_passes(self):
        actual, mismatch = version_review({'selected': {'progid': 'GStarCAD.Application.22'}}, 'Icad.Application.1')
        self.assertEqual(actual, 'GStarCAD.Application.22')
        self.assertIn('不能算指定版本', mismatch)
        self.assertIn('Icad.Application.1', mismatch)

    def test_missing_progid_does_not_guess_a_version(self):
        for report in ({}, {'selected': None}, {'selected': {}}, {'selected': {'progid': None}}):
            with self.subTest(report=report), self.assertRaisesRegex(ValueError, '確切CAD ProgID'):
                version_review(report, 'Icad.Application.1')

    def test_latest_is_not_native_version_evidence(self):
        with self.assertRaisesRegex(ValueError, '確切CAD ProgID'):
            version_review({'selected': {'progid': 'latest'}}, 'Icad.Application.1')


if __name__ == '__main__':
    unittest.main()
