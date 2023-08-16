#!/bin/bash

trap "echo; exit" INT
trap "echo; exit" HUP

PARENT_DIR=$( echo $(dirname "$(dirname "$(realpath "${BASH_SOURCE[0]}")")") )
# generate .env file from .env.example if it does not exist
# https://stackoverflow.com/a/47677632/3208553
if [ -e .env ]
then
    echo ".env file exists"
else
	echo "generating .env file from .env.example since it does not exist";
	touch .env && cp .env.example .env;
fi

# assign fallback values for environment variables from .env.example incase
# not declared in .env file. alternative approach is `echo ${X:=$X_FALLBACK}`
source $PARENT_DIR/.env.example
source $PARENT_DIR/.env

printf "\n*** Started building Docker container."
printf "\n*** Please wait... \n***"

# https://stackoverflow.com/a/25554904/3208553
set +e
bash -e <<TRY
  docker build -f ${PARENT_DIR}/docker/Dockerfile -t helios-test:latest ./
TRY
if [ $? -ne 0 ]; then
	printf "\n*** Detected error running 'docker build'. Trying 'docker buildx' instead...\n"
	docker buildx build -f {PARENT_DIR}/docker/Dockerfile -t helios-test:latest ./
fi

docker run -it -d \
	--env-file "${PARENT_DIR}/.env" \
	--hostname helios-test \
	--name helios-test \
	--publish 0.0.0.0:8545:8545 \
	--volume ${PARENT_DIR}:/opt:rw \
	helios-test:latest
if [ $? -ne 0 ]; then
    kill "$PPID"; exit 1;
fi
CONTAINER_ID=$(docker ps -n=1 -q)
printf "\n*** Finished building Docker container ${CONTAINER_ID}.\n\n"

docker ps -a

printf "\n*** Entering shell of Docker container...\n"
docker exec -it helios-test /bin/bash
if [ $? -ne 0 ]; then
    kill "$PPID"; exit 1;
fi
