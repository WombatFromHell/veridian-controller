#!/usr/bin/env bash

get_version() {
	VERSION=$(grep '^version =' Cargo.toml | sed 's/version = "//' | sed 's/"//')
}
get_version

APPIMAGE_EXTRACT_AND_RUN=1 cargo appimage

artifact_name_path="/app/target/appimage"
artifact_name="veridian-controller"
artifact_ext=".AppImage"

full_binary_path="${artifact_name_path}/${artifact_name}${artifact_ext}"
full_versioned_path="${artifact_name_path}/${artifact_name}_${VERSION}${artifact_ext}"
versioned_path="${artifact_name}_${VERSION}${artifact_ext}"

mv -f "${full_binary_path}" "${full_versioned_path}"
echo "" && echo "AppImage release build created at: \"${full_versioned_path}\""

cd /app/target/appimage || exit
sha256sum "${versioned_path}" >"${versioned_path}.sha256"

if [[ "$CI" == false ]]; then
    # package to tar.gz when running locally
    tar -cvzf veridian-controller_"${VERSION}".tar.gz ./*
fi