#!/usr/bin/env bash

CI="${CI:-false}"
IMAGE_NAME="veridian"

selinux_status() {
	ENFORCING=false
	if command -v getenforce; then
		enforcing_status=$(getenforce)
		if [ "$enforcing_status" == "Enforcing" ]; then
			ENFORCING=true
		fi
	fi
}

selinux_status

RELABEL=""
if [[ "$ENFORCING" == true ]]; then
	RELABEL=":z"
fi

rm -rf ./target/appimage &&
	mkdir -p ./target

docker build --build-arg CI="$CI" -t $IMAGE_NAME .
docker run --rm \
	-v "./target:/app/target${RELABEL}" \
	--name $IMAGE_NAME $IMAGE_NAME
