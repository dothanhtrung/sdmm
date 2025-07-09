<div align="center">

Stable Diffusion Models Manager
===============================

[![pipeline status](https://gitlab.com/kimtinh/sdmm/badges/master/pipeline.svg)](https://gitlab.com/kimtinh/sdmm/-/commits/master)

[![Gitlab](https://img.shields.io/badge/gitlab-%23181717.svg?style=for-the-badge&logo=gitlab&logoColor=white)](https://gitlab.com/kimtinh/sdmm)
[![Github](https://img.shields.io/badge/github-%23121011.svg?style=for-the-badge&logo=github&logoColor=white)](https://github.com/dothanhtrung/sdmm)

![](./preview.png)

</div>

Standalone web app to manage your local Stable Diffusion models.

Features:
* [x] Manage model with tag.
* [x] Get preview image and model info from Civitai by hash.
* [x] Download from Civitai

How to run
----------

See the sample config at [sdmm-config-sample.ron](./sdmm-config-sample.ron) and update to your need.

Run the web server:
```shell
./sdmm -c ./path/to/config.ron
```

> Note: Put the [res](./res) folder in same directory with binary `sdmm`.

Install
-------

Get the prebuilt binary in Release page or build it with `cargo`.

Build the application:
```shell
cargo build --release
```

Update CSS:
```shell
cd res
npm install tailwindcss @tailwindcss/cli 
npx @tailwindcss/cli -i ./css/tailwind_input.css -o ./css/tailwind_output.min.css --watch --minify
```