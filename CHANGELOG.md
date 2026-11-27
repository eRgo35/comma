# Changelog

## [0.5.1](https://github.com/eRgo35/comma/compare/comma-v0.5.0...comma-v0.5.1) (2026-11-27)


### Bug Fixes

* add Clink history in-process, keeping quotes ([#22](https://github.com/eRgo35/comma/issues/22)) ([3e1f642](https://github.com/eRgo35/comma/commit/3e1f642d5658efa2112a98995967a8362072345a))
* allow 10 s for an API request, 3 s to connect ([#21](https://github.com/eRgo35/comma/issues/21)) ([4a04596](https://github.com/eRgo35/comma/commit/4a045962cd29f6cd0c0271473bc5c8b2fc99ecde)), closes [#17](https://github.com/eRgo35/comma/issues/17)
* bash risk check flags git reset --hard, git clean -f, truncate, recursive chmod/chown and shutdown ([#20](https://github.com/eRgo35/comma/issues/20)) ([f1f9040](https://github.com/eRgo35/comma/commit/f1f9040509808e0ea375c27cc99ca313c881365a)), closes [#19](https://github.com/eRgo35/comma/issues/19)

## [0.5.0](https://github.com/eRgo35/comma/compare/comma-v0.4.0...comma-v0.5.0) (2026-11-26)


### Features

* ask the API for schema-constrained JSON replies ([#14](https://github.com/eRgo35/comma/issues/14)) ([4927615](https://github.com/eRgo35/comma/commit/492761565048b0a93470d50a0e7a35cd89fd09d3))
* retry the API once on 429 or 5xx before falling back to claude -p ([#13](https://github.com/eRgo35/comma/issues/13)) ([b6614d8](https://github.com/eRgo35/comma/commit/b6614d887bef6c889158f5a6a48b0cc5bd422710)), closes [#10](https://github.com/eRgo35/comma/issues/10)


### Performance Improvements

* smaller release binary (strip, LTO, no gzip) ([#12](https://github.com/eRgo35/comma/issues/12)) ([f6dd002](https://github.com/eRgo35/comma/commit/f6dd00279a688e58724d87b88858fe85ec46f5c6)), closes [#11](https://github.com/eRgo35/comma/issues/11)

## [0.4.0](https://github.com/eRgo35/comma/compare/comma-v0.3.0...comma-v0.4.0) (2026-11-25)


### Bug Fixes

* bash risk check sees $(...) and backticks inside double quotes ([4b572fd](https://github.com/eRgo35/comma/commit/4b572fd7a1ae34e2f41ae080d03b33fc08f9222d))
* read claude's output while it runs so a long reply can't stall the fallback ([93c8083](https://github.com/eRgo35/comma/commit/93c8083f6353f843f0b6aab6de98dce9bd8a5c44))
* run the claude fallback without MCP servers, settings, hooks or saved sessions ([c8e7bde](https://github.com/eRgo35/comma/commit/c8e7bde7baa79b2ba010278fb23fc90e3cb20aca))

## [0.3.0](https://github.com/eRgo35/comma/compare/comma-v0.2.0...comma-v0.3.0) (2026-11-25)


### Features

* Clink hook for cmd.exe (comma init cmd) ([31ea377](https://github.com/eRgo35/comma/commit/31ea3770476b9696417b98080073af2a6787eac1))
* cmd shell mode and request from COMMA_REQUEST ([6be6912](https://github.com/eRgo35/comma/commit/6be691249b7168a19876bd85cc072ce7898a8fd6))
* risk warnings for cmd and PowerShell commands ([3838d9e](https://github.com/eRgo35/comma/commit/3838d9e13a25a7987054cc055bbe84a152573f85))


### Bug Fixes

* build and test on Windows ([6816f33](https://github.com/eRgo35/comma/commit/6816f33b1f5f01662356e43ab80f1675c15dad1c))
* cmd risk check catches [@del](https://github.com/del), del/q, sudo, robocopy /MIR, reg delete, downloaded scripts ([77688f7](https://github.com/eRgo35/comma/commit/77688f70bcf7633cd1d6a1f04570b824e130fe9a))
* hook also handles Ctrl-J and never runs a comma from the current directory ([55efcf0](https://github.com/eRgo35/comma/commit/55efcf0bff3362440e834240afeab3c3631beeb8))
* static CRT for the Windows release, fail CI if Clink tests can't run ([beeb7c7](https://github.com/eRgo35/comma/commit/beeb7c7c7a374fc21e490814a6117833d3f91f23))

## [0.2.0](https://github.com/eRgo35/comma/compare/comma-v0.1.0...comma-v0.2.0) (2026-11-08)


### Features

* warn locally about clearly risky commands the model didn't flag ([d2918e6](https://github.com/eRgo35/comma/commit/d2918e66ae12bc212861e271d5a9c73ef6b6b624))


### Bug Fixes

* disable thinking so API replies aren't truncated ([4c11b49](https://github.com/eRgo35/comma/commit/4c11b4912d2d103411e4edbc8c924e4c6073a18b))
* keep COMMA_API_KEY out of other programs' environment ([4ebc659](https://github.com/eRgo35/comma/commit/4ebc65999589ca10622d7118e5f401f7db954c15))
* pass the request after -- so the claude fallback can't read it as a flag ([c25656e](https://github.com/eRgo35/comma/commit/c25656e9c5fe25a7efbc63516b0115a12b9c1bd4))
* reject multi-line commands and strip escape codes from model output ([c73b759](https://github.com/eRgo35/comma/commit/c73b7592a290cf00cdad0983e1e865380c4df67f))
* say when the reply was cut off or refused instead of 'no text' ([c3cde2f](https://github.com/eRgo35/comma/commit/c3cde2f89dc6d5027e110498513191c8a46f7128))
* show claude's actual error instead of its first stderr line ([1d89c76](https://github.com/eRgo35/comma/commit/1d89c76195e88a77c94a33daf5e2f7fddc393fe9))
* time out the claude fallback after 30 seconds ([0a95440](https://github.com/eRgo35/comma/commit/0a954405024a88268adb409bfc9d8513ee9a8cfc))

## 0.1.0 (2026-11-04)


### Features

* anthropic api backend with claude -p fallback ([4106b89](https://github.com/eRgo35/comma/commit/4106b89093fe7fcf04926a302779f07b5b36cc49))
* build prompt and context ([e2e4e4e](https://github.com/eRgo35/comma/commit/e2e4e4e666418831f672049a80d116e212a2c8ab))
* comma CLI and bash Enter hook ([3614f44](https://github.com/eRgo35/comma/commit/3614f44a0828e9a88105e4ed99ed66b7b054a6b5))
* parse model replies ([81c5bc7](https://github.com/eRgo35/comma/commit/81c5bc730860d4c82ed5a4cd3e6c3d3bb197604d))
* read API key from COMMA_API_KEY ([ce2b9e2](https://github.com/eRgo35/comma/commit/ce2b9e2fbcbaf5995c2e84ef5d0105f4e06b61c3))


### Bug Fixes

* keep $_, skip heredoc lines, support vi mode in Enter hook ([310fac9](https://github.com/eRgo35/comma/commit/310fac9313037e7399dfd742ca295668e3f0bfd3))
