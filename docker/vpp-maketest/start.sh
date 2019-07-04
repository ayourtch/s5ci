#!/bin/sh
# -evx
EXECSTART=`date`
echo Arguments: $1 $2 $3 $4 $5
echo EXECSTART: $EXECSTART

. /secrets/env-vars.env
echo ====== ENV START ======
env
echo ====== ENV END ======

export CCACHE_DIR=/CCACHE
git clone ${ARG_VPP_GIT_URL} vpp
cd vpp
echo ====== GIT BEFORE PULL ======
git log HEAD~3..
git pull
echo ====== GIT AFTER PULL ======
git log HEAD~3..
git fetch ${ARG_VPP_GIT_URL} ${ARG_FETCH_REF} && git checkout FETCH_HEAD
echo ====== GIT AFTER CHECKOUT ======
git log HEAD~3..
if TEST=${ARG_TEST} TEST_JOBS=auto UNATTENDED=y CACHE_OUTPUT=0 make install-dep test; then
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
exit ${EXIT_CODE}

# while true; do date; sleep 1; done

