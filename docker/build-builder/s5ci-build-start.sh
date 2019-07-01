#!/bin/sh

. /secrets/env-vars.env

# make directory for logs
mkdir ${LOG_DIR}

echo "$(date): EXECSTART" >>${TIMELINE_LOG}
mkdir -p ~/.ssh

export NETWORK_LOG="${LOG_DIR}/network.log"
date >>${NETWORK_LOG}
curl http://s5ci-images.myvpp.net >>${NETWORK_LOG}
echo "end of net" >>${NETWORK_LOG}

while true; do curl http://s5ci-images.myvpp.net >> ${INITIAL_RSYNC_LOG} && break; date; sleep 1; done
echo "$(date): connectivity is ok" >>${TIMELINE_LOG}

echo START INSTALL
echo Check Docker
docker ps
echo Docker OK

#### run the build process

curl https://sh.rustup.rs -sSf | sh -s -- -y
export PATH="$HOME/.cargo/bin:$PATH"

# install the prerequisites
cargo install diesel_cli --no-default-features --features "postgres sqlite"
rustup component add rustfmt --toolchain stable-x86_64-unknown-linux-gnu

cd /s5ci-build
# make it
git clone https://github.com/ayourtch/s5ci
cd s5ci
make
mkdir db
diesel migration run --database-url db/s5ci.sqlite3
make regen-db
make


cd docker
cp /secrets/s5ci-publish-image .
cp /secrets/s5ci-tweak-configs .
bash s5ci-build


