#!/usr/bin/env just --justfile

css:
  cd res && npx @tailwindcss/cli -i ./css/tailwind_input.css -o ./css/tailwind_output.min.css --watch --minify