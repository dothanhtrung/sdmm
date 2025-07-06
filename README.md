Stable Diffusion Models Manager
===============================

Standalone web app to manage your local Stable Diffusion models.

Features:
* [x] Manage model with tag.
* [x] Get preview image and model info from Civitai by hash.
* [x] Download from Civitai

![](./preview.png)

How to run
----------

See the sample config at [sdmm-config-sample.ron](./sdmm-config-sample.ron) and update to your need.

Run the web server:
```shell
./sdmm -c ./path/to/config.ron
```

How to build
------------

```shell
cargo build --release
```