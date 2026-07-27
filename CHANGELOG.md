<a name="v0.2.1"></a>

## [v0.2.1](https://github.com/MuntasirSZN/getquotes/compare/v0.2.0...v0.2.1) (2026-07-21)

### ✨ Features

- much more faster
- who doesnt love more speed


### 🐞 Bug Fixes

- add ignore for timeout too
- fixes
- kani and clippy
- last commit issues


### ⌛ Performance Improvements

- one function dispatch
- more?
- fix codspeed regresses


<a name="v0.2.0"></a>

## [v0.2.0](https://github.com/MuntasirSZN/getquotes/compare/v0.1.0...v0.2.0) (2026-07-16)

### 🐞 Bug Fixes

- issues
- use changelogithub


<a name="v0.1.0"></a>

## v0.1.0 (2026-07-14)

### ✨ Features

- multi-ISA dispatch for simd_skip_equal via cpufeatures
- add all x86_64 ISA backends for simd_skip_equal
- add normalization
- kani stuff
- add WASM target support and AVX-512BW SIMD ASCII detection
- fuzz targets


### 🐞 Bug Fixes

- cargo.toml
- gate cpufeatures::new! behind cfg target_arch x86_64
- require 1.89 as thats where gfni was stabilized
- doctests
- some more proptest
- mutants again
- more mutants
- macos and windows
- windows again
- mutants
- mutants
- neon masking issue found by proptest
- last
- last 2.0


### ⌛ Performance Improvements

- use tzcnt from PMOVMSKB to skip scalar rescan in simd_skip_equal


### 📝 Code Refactoring

- remove collator, add miri, fix ub


