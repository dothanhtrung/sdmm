#!/usr/bin/env just --justfile

css:
  cd res && npx @tailwindcss/cli -i ./css/tailwind_input.css -o ./css/tailwind_output.min.css --watch --minify

windows:
  cargo build --target=x86_64-pc-windows-gnu --release

release:
  cargo build --release
  rm -rf output/
  mkdir -p output/sdmm/res
  cp target/release/sdmm output/sdmm
  cp -r res/html output/sdmm/res/
  cp -r res/css output/sdmm/res/
  cp -r res/assets output/sdmm/res/
  cd output && tar cJvf sdmm_linux.tar.xz sdmm