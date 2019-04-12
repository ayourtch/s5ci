# Download base ubuntu image
FROM ubuntu:18.04

RUN apt-get update

# RUN apt-get install -y build-essential git && rm -rf /var/lib/apt/lists/*
RUN apt-get install -y build-essential git sudo 

COPY start.sh /start.sh

CMD ["./start.sh"]

# EXPOSE 1234


