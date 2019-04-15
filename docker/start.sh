#!/bin/sh -evx
EXECSTART=`date`
echo Arguments: $1 $2 $3 $4 $5
echo EXECSTART: $EXECSTART
git clone http://testgerrit.myvpp.net/r/testvpp
cd testvpp
git fetch http://testgerrit.myvpp.net/r/testvpp $1 && git checkout FETCH_HEAD
UNATTENDED=y make install-dep test
EXECEND=`date`
echo EXECEND: $EXECEND

# while true; do date; sleep 1; done

