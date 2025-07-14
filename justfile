#!/usr/bin/env just --justfile

css:
    cd res && npx @tailwindcss/cli -i ./css/tailwind_input.css -o ./css/tailwind_output.min.css --watch --minify

windows:
    cargo build --target=x86_64-pc-windows-gnu --release
    rm -rf output/windows
    mkdir -p output/windows/sdmm/res
    cp -r res/html res/css res/assets output/windows/sdmm/res
    cd output/windows && zip sdmm_windows.zip sdmm

linux:
    cargo build --target=x86_64-unknown-linux-musl --release
    rm -rf output/linux
    mkdir -p output/linux/sdmm/res
    cp -r res/html res/css res/assets output/linux/sdmm/res
    cd output/linux && tar cJvf sdmm_linux.tar.xz sdmm

release: windows linux
