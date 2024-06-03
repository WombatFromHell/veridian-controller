#!/usr/bin/env bash

IMAGE_NAME="veridian"

ARTIFACT_PATH="./target/appimage"
ARTIFACT="veridian-controller"
ARTIFACT_EXT=".AppImage"

get_version() {
	VERSION=$(grep '^version =' Cargo.toml | sed 's/version = "//' | sed 's/"//')
}

selinux_status() {
	ENFORCING=false
	if command -v getenforce; then
		enforcing_status=$(getenforce)
		if [ "$enforcing_status" == "Enforcing" ]; then
			ENFORCING=true
		fi
	fi
}

get_version
selinux_status

RELABEL=""
if [[ "$ENFORCING" == true ]]; then
	RELABEL=":z"
fi

rm -rf ./target/appimage &&
	mkdir -p ./target

docker build -t $IMAGE_NAME .
docker run --rm \
	-v "./target:/app/target${RELABEL}" \
	--name $IMAGE_NAME $IMAGE_NAME
# rename the release artifact with the current version from Cargo.toml
mv -f \
	"${ARTIFACT_PATH}/${ARTIFACT}${ARTIFACT_EXT}" \
	"${ARTIFACT_PATH}/${ARTIFACT}_${VERSION}${ARTIFACT_EXT}"
echo "" && echo "AppImage release build created at: \"${ARTIFACT_PATH}/${ARTIFACT}_${VERSION}${ARTIFACT_EXT}\""
