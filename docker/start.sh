#!/bin/sh -evx
EXECSTART=`date`
echo Arguments: $1 $2 $3 $4 $5
echo EXECSTART: $EXECSTART
# git clone http://testgerrit.myvpp.net/r/testvpp
cd testvpp
git pull
git fetch http://testgerrit.myvpp.net/r/testvpp $1 && git checkout FETCH_HEAD
if TEST=$2 UNATTENDED=y make install-dep test; then
	echo Inside docker: success
else
	EXIT_CODE=$?
	echo Inside docker: failure, exit code ${EXIT_CODE}
	cd /
	for CORE in $(find /tmp/vpp* -name core); do
		BINFILE=$(gdb -c ${CORE} -ex quit | grep 'Core was generated' | awk '{ print $5; }' | sed -e s/\`//g)
		echo ====================================================== DECODE CORE: ${CORE}
		gdb ${BINFILE} ${CORE} -ex 'source -v gdb-commands' -ex quit 
	done
fi

EXECEND=`date`
echo EXECEND: $EXECEND

# while true; do date; sleep 1; done

