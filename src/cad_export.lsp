; 固定唯讀效能原型。由測試工具填入輸出檔與 nonce；不接受模型產生的 LISP。
; 不使用 command、entmod、entmake、Save、Close 或全圖系統變數設定。
; 每行是一個 JSON 陣列；實數以 16 位小數輸出，避免 prin1 預設精度造成失真。
; ANSI LISP的substr按位元組切字；Big5「餐／許」的尾碼可能是92，
; 必須把完整雙位元組字元一起寫出，不能把尾碼當成JSON反斜線。
; 範圍由固定Python匯出器查Windows字碼頁提供；Unicode引擎傳入空範圍。
(defun lm-ai-dbcs-lead-p (n / ranges found)
  (setq ranges '@@LEAD_BYTES@@ found nil)
  (while ranges
    (if (and (<= (car ranges) n) (<= n (cadr ranges))) (setq found T))
    (setq ranges (cddr ranges)))
  found)

(defun lm-ai-json-string (s f / i c n size)
  (princ "\"" f)
  (setq i 1 size (strlen s))
  (while (<= i size)
    (setq c (substr s i 1) n (ascii c))
    (cond
      ((and (>= n 128) (< i size) (lm-ai-dbcs-lead-p n))
        (princ (substr s i 2) f)
        (setq i (1+ i)))
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

(defun lm-ai-native-value (v)
  (if (= (type v) 'ENAME) (list "HANDLE" (cdr (assoc 5 (entget v)))) v))

(defun lm-ai-native-fields (data / pair fields)
  (foreach pair data
    (if (/= (car pair) -1)
      (setq fields (cons (list (car pair) (lm-ai-native-value (cdr pair))) fields))))
  (reverse fields))

(defun lm-ai-progress (i / p)
  (setq p (open "@@PROGRESS@@" "w"))
  (if p (progn (lm-ai-json-line (list "PROGRESS" "@@NONCE@@" i) p) (close p))))

(defun lm-ai-export-probe (/ f ss i e data fields child children kind)
  (setq f (open "@@OUTPUT@@" "w"))
  (if f
    (progn
      (lm-ai-json-line
        (list "HEADER" 2 "@@NONCE@@" (getvar "DWGNAME") (getvar "DWGPREFIX")
              (getvar "INSUNITS") (getvar "DBMOD") (getvar "DWGCODEPAGE")) f)
      (setq ss (ssget "_X" '((67 . 0))) i 0)
      ; 保留重複群組（MTEXT 分段、折線頂點等），不以 assoc 擅自折疊。
      (lm-ai-progress 0)
      (if ss
        (repeat (sslength ss)
          (setq e (ssname ss i) data (entget e) kind (cdr (assoc 0 data)) children nil)
          ; 傳統折線頂點與圖塊屬性是子圖元，不能只讀主圖元而遺漏內容。
          (if (or (= kind "POLYLINE") (and (= kind "INSERT") (= 1 (cdr (assoc 66 data)))))
            (progn
              (setq child (entnext e))
              (while (and child (/= "SEQEND" (cdr (assoc 0 (entget child)))))
                (if (or (not @@HANDLES@@) (= "ATTRIB" (cdr (assoc 0 (entget child)))))
                  (setq children (cons
                    (if @@HANDLES@@ (list (list 0 "ATTRIB") (list 5 (cdr (assoc 5 (entget child)))))
                        (lm-ai-native-fields (entget child))) children)))
                (setq child (entnext child)))))
          (lm-ai-json-line (list "ENTITY"
            (if @@HANDLES@@ (list (list 0 kind) (list 5 (cdr (assoc 5 data)))) (lm-ai-native-fields data))
            (reverse children)) f)
          (setq i (1+ i))
          (if (= 0 (rem i 250)) (lm-ai-progress i))))
      (lm-ai-json-line (list "COMPLETE" "@@NONCE@@" i (getvar "DBMOD")) f)
      (close f)))
  (lm-ai-progress i)
  (princ))

(lm-ai-export-probe)
