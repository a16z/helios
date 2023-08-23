#!/bin/bash

trap "echo; exit" INT
trap "echo; exit" HUP

unknown_dist () {
  printf "\n*** This OS is not supported with this script at present."
  printf "\n*** Please try to manually install the 'realpath' binary and run the script again."
  printf "\n*** Please also create an issue at https://github.com/a16z/helios/issues/new with details\n"
  printf "\n*** of your OS that was missing the 'realpath' binary so we may improve this script."

  exit 1;
}

if ! which realpath >/dev/null 2>&1; then
  printf "\n*** Detected that the 'realpath' binary required by this script is not available"
  # install `realpath` on macOS from Homebrew `coreutils`
  # reference: https://unix.stackexchange.com/a/336138/269147
  if [[ "$OSTYPE" == "darwin"* ]]; then
    set -e
    printf "\n*** Mac OS (Darwin) detected."
    printf "\n*** Installation of Homebrew and coreutils."
    printf "\n*** Press SPACE to continue the installation or Ctrl+C to exit..."
    while true; do
      read -n1 -r
      [[ $REPLY == ' ' ]] && break
    done
    printf "\n*** Continuing..."

    if ! which brew >/dev/null 2>&1; then
      printf "\n*** Installing Homebrew..."
      /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install.sh)"
    fi

    printf "\n*** Updating Homebrew..."
    # cannot run this as sudo user
    brew update
    printf "\n*** Installing coreutils..."
    brew install coreutils
  # credits: https://getsubstrate.io/
  elif [[ "$OSTYPE" == "linux-gnu" ]]; then
    set -e
    if [[ $(whoami) == "root" ]]; then
      MAKE_ME_ROOT=
    else
      MAKE_ME_ROOT=sudo
    fi

    if [ -f /etc/debian_version ]; then
      echo "Ubuntu/Debian Linux detected."
      $MAKE_ME_ROOT apt update
      $MAKE_ME_ROOT apt install -y realpath
    else
      printf "\n*** Unknown Linux distribution."
      unknown_dist
    fi
  else
    printf "\n*** Unknown distribution."
    unknown_dist
  fi
fi

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
  docker build -f ${PARENT_DIR}/docker/Dockerfile -t helios:latest ./
TRY
if [ $? -ne 0 ]; then
  printf "\n*** Detected error running 'docker build'. Trying 'docker buildx' instead...\n"
  docker buildx build -f {PARENT_DIR}/docker/Dockerfile -t helios:latest ./
fi

docker run -it -d \
  --env-file "${PARENT_DIR}/.env" \
  --hostname helios \
  --name helios \
  --publish 0.0.0.0:8545:8545 \
  --publish 0.0.0.0:9001:9001 \
  --publish 0.0.0.0:9002:9002 \
  --volume ${PARENT_DIR}:/root/helios:rw \
  helios:latest
if [ $? -ne 0 ]; then
  kill "$PPID"; exit 1;
fi
CONTAINER_ID=$(docker ps -n=1 -q)
printf "\n*** Finished building Docker container ${CONTAINER_ID}.\n\n"

docker ps -a

printf "\n*** Entering shell of Docker container...\n"
docker exec -it helios /bin/bash
if [ $? -ne 0 ]; then
  kill "$PPID"; exit 1;
fi
