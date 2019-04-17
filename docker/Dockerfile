# Download base ubuntu image
FROM ubuntu:18.04

RUN apt-get update

# RUN apt-get install -y build-essential git && rm -rf /var/lib/apt/lists/*
RUN apt-get install -y build-essential git sudo gdb
# RUN git clone http://testgerrit.myvpp.net/r/testvpp
# RUN (cd testvpp; git log HEAD~3.. 1>&2)
# RUN (cd testvpp; UNATTENDED=y make install-dep)
# RUN (cd testvpp; make build-release)
RUN mkdir /CCACHE

COPY  start.sh /start.sh
COPY  gdb-commands /gdb-commands

CMD ["./start.sh"]

# EXPOSE 1234


