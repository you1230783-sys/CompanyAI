# Bundled Rider–Waite tarot images

The 78 faces are Pamela Colman Smith's 1909 Rider–Waite deck, distributed by
https://github.com/mixvlad/TarotCards at commit
`840b84d012c6b74dc1634fa2d00d1b21f5b28093`.
The upstream deck metadata identifies the artwork as **Public Domain** and
credits the scans to Steve-P.org. The upstream README, metadata, SOURCES and
LICENSE are retained in this directory; the repository's software license is
distinct from the artwork's public-domain attribution.

Only the 78 cards in `tarot/rider-waite/720px` are included. Cover images are not
cards and were excluded. Source files were verified against Git blob SHA-1
identifiers before processing. `scripts/Prepare-TarotAssets.ps1` scales each
complete image without cropping into a 360 × 600 JPEG at quality 86. Output
hashes, source filenames and the fixed source commit are in
`ui/tarot/manifest.json`.

The deep-blue card back is supplied by the user and is not from that repository.
Chinese names and upright/reversed descriptions are maintained separately in
`ui/tarot-data.json`. Reversal rotates the image at display time.
