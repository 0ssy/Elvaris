# Application Support Matrix (Milestone 21.9)

| Application | Origin | Arch | Runtime | Support Level | Permissions | Status | Notes |
|-------------|--------|------|---------|---------------|-------------|--------|-------|
| hello-world (native) | native | x86_64/aarch64 | native | Native integration | none/minimal | Reference | Baseline |
| file-util (portable) | portable | x86_64/aarch64 | native/portable | Builds/Usable | filesystem.read/write | Template | Planned |
| posix-util | posix | x86_64/aarch64 | posix | Experimental/Usable | varies | Template | Planned |
| linux-cli | linux | x86_64 | linux-compat (recompile/translate) | Experimental | varies | Template | Planned |
| linux-net | linux | x86_64 | linux-compat | Experimental | network.client/server | Template | Planned |
| linux-build | linux | x86_64 | linux-compat | Experimental | filesystem | Template | Planned |
| linux-gui | linux | x86_64 | linux-compat+gfx | Experimental | graphics/window | Conditional | If graphics ready |
