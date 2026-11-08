# Changelog

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
