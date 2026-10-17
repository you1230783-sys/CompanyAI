; 固定唯讀效能原型。由測試工具填入輸出檔與 nonce；不接受模型產生的 LISP。
; 不使用 command、entmod、entmake、Save、Close 或全圖系統變數設定。
; 每行是一個 JSON 陣列；實數以 16 位小數輸出，避免 prin1 預設精度造成失真。
(defun lm-ai-json-string (s f / i c n)
  (princ "\"" f)
  (setq i 1)
  (while (<= i (strlen s))
    (setq c (substr s i 1) n (ascii c))
    (cond
      ((= n 34) (princ "\\\"" f))
      ((= n 92) (princ "\\\\" f))
      ((= n 10) (princ "\\n" f))
      ((= n 13) (princ "\\r" f))
      ((= n 9) (princ "\\t" f))
      ((< n 32)
        (princ "\\u00" f)
        (princ (substr "0123456789abcdef" (1+ (fix (/ n 16))) 1) f)
        (princ (substr "0123456789abcdef" (1+ (rem n 16)) 1) f))
      (T (princ c f)))
    (setq i (1+ i)))
  (princ "\"" f))

(defun lm-ai-json-value (v f / first item s)
  (cond
    ((null v) (princ "null" f))
    ((= (type v) 'STR) (lm-ai-json-string v f))
    ((= (type v) 'INT) (princ (itoa v) f))
    ((= (type v) 'REAL)
      (setq s (rtos v 2 16))
      ; DIMZIN 可能省略前導零；只修正輸出文字，不修改圖面設定。
      (if (= (substr s 1 1) ".") (setq s (strcat "0" s)))
      (if (= (substr s 1 2) "-.") (setq s (strcat "-0" (substr s 2))))
      (princ s f))
    ((= (type v) 'LIST)
      (princ "[" f) (setq first T)
      (foreach item v
        (if (not first) (princ "," f))
        (setq first nil)
        (lm-ai-json-value item f))
      (princ "]" f))
    (T (princ "null" f))))

(defun lm-ai-json-line (v f)
  (lm-ai-json-value v f)
  (write-line "" f))

(defun lm-ai-export-probe (/ f ss i data pair fields codes)
  (setq f (open "@@OUTPUT@@" "w"))
  (if f
    (progn
      (lm-ai-json-line
        (list "HEADER" 1 "@@NONCE@@" (getvar "DWGNAME") (getvar "DWGPREFIX")
              (getvar "INSUNITS") (getvar "DBMOD")) f)
      (setq ss (ssget "_X" '((67 . 0))) i 0)
      ; 保留重複群組（MTEXT 分段、折線頂點等），不以 assoc 擅自折疊。
      (setq codes '(0 5 8 62 6 48 39 10 11 12 13 14 15 16 17 40 41 42 43 44 45
                    50 51 52 53 70 71 72 73 74 75 76 77 90 91 92 93 210 1 2 3 7 38))
      (if ss
        (repeat (sslength ss)
          (setq data (entget (ssname ss i)) fields nil)
          (foreach pair data
            (if (member (car pair) codes)
              (setq fields (cons (list (car pair) (cdr pair)) fields))))
          (lm-ai-json-line (list "ENTITY" (reverse fields)) f)
          (setq i (1+ i))))
      (lm-ai-json-line (list "COMPLETE" "@@NONCE@@" i (getvar "DBMOD")) f)
      (close f)))
  (princ))

(lm-ai-export-probe)
