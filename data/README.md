# data/

Training-data pipeline for Tez-Coder. Filled in from **Phase 3** (v1) and **Phase 5**.

Pipeline: seed tasks → candidates → `tez check` + tests (rejection sampling) → `tez fmt` → dataset, plus repair triples and mutation data. TezBench tasks are never included. Record provenance and license per sample.

Large outputs go to `data/raw/` and `data/out/` (git-ignored).
