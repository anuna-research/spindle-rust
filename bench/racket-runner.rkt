#lang racket
;; Benchmark runner for spindle-racket
;; Usage: racket racket-runner.rkt <spl-file>

(require racket/cmdline
         racket/format
         "../../spindle-racket/src/spindle.rkt")

(define input-file (make-parameter #f))

(command-line
 #:program "racket-runner"
 #:args (file)
 (input-file file))

(define (run-benchmark)
  (define content (file->string (input-file)))
  (define theory (parse-spl content))

  ;; Time the reasoning
  (define start (current-inexact-milliseconds))
  (define conclusions (reason theory #:mode 'standard))
  (define end (current-inexact-milliseconds))

  ;; Output JSON result
  (printf "{\"time_ms\": ~a, \"conclusions\": ~a, \"mode\": \"~a\"}\n"
          (~r (- end start) #:precision 3)
          (length conclusions)
          "standard"))

(run-benchmark)
