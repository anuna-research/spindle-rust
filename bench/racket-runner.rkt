#lang racket
;; Benchmark runner for spindle-racket
;; Usage: racket racket-runner.rkt <dfl-file> [--scalable]

(require racket/cmdline
         racket/format
         "../../spindle-racket/src/spindle.rkt")

(define scalable-mode? (make-parameter #f))
(define input-file (make-parameter #f))

(command-line
 #:program "racket-runner"
 #:once-each
 [("--scalable" "-s") "Use scalable reasoning mode"
  (scalable-mode? #t)]
 #:args (file)
 (input-file file))

(define (run-benchmark)
  (define content (file->string (input-file)))
  (define theory (parse-dfl content))

  ;; Time the reasoning
  (define start (current-inexact-milliseconds))
  (define conclusions
    (reason theory #:mode (if (scalable-mode?) 'scalable 'standard)))
  (define end (current-inexact-milliseconds))

  ;; Output JSON result
  (printf "{\"time_ms\": ~a, \"conclusions\": ~a, \"mode\": \"~a\"}\n"
          (~r (- end start) #:precision 3)
          (length conclusions)
          (if (scalable-mode?) "scalable" "standard")))

(run-benchmark)
