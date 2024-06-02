#!/usr/bin/env bash

cargo build -r
APPIMAGE_EXTRACT_AND_RUN=1 cargo appimage