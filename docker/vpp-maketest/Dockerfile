# Download base ubuntu image
FROM ubuntu:18.04
# Force the full refresh 
RUN touch /built-on-2020-02-06.txt
#
RUN apt-get update
RUN apt-get upgrade -y && true
# VPP prerequisites
RUN apt-get install -y curl build-essential autoconf automake ccache debhelper dkms git libtool libapr1-dev dh-systemd
RUN apt-get install -y libconfuse-dev git-review exuberant-ctags cscope pkg-config
RUN apt-get install -y lcov chrpath autoconf indent clang-format libnuma-dev
RUN apt-get install -y python-all python-dev python-virtualenv python-pip libffi6 check
RUN apt-get install -y libboost-all-dev libffi-dev python-ply libmbedtls-dev
RUN apt-get install -y cmake ninja-build uuid-dev libssl-dev python3-jsonschema python3-yaml


# RUN apt-get install -y build-essential git && rm -rf /var/lib/apt/lists/*
RUN apt-get install -y build-essential git sudo gdb
RUN apt-get install -y iperf3 && true
# RUN git clone http://testgerrit.myvpp.net/r/testvpp
# RUN (cd testvpp; git log HEAD~3.. 1>&2)
# RUN (cd testvpp; UNATTENDED=y make install-dep)
# RUN (cd testvpp; make build-release)
RUN mkdir /CCACHE

COPY  start.sh /start.sh
COPY  decodecores /decodecores
COPY  gdb-commands /gdb-commands

RUN touch /last-built.txt

CMD ["./start.sh"]

# EXPOSE 1234


