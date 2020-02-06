#!/bin/sh
# -evx
EXECSTART=`date`
echo Arguments: $1 $2 $3 $4 $5
echo EXECSTART: $EXECSTART
ls -al /

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
echo ====== DIFF of the last change ======
git diff HEAD~1..HEAD
echo ====== END DIFF =====
# CACHE_OUTPUT=0 borks on what seems to be big debug CLI outputs
# if TEST=${ARG_TEST} TEST_JOBS=auto UNATTENDED=y CACHE_OUTPUT=0 make install-dep test; then
if [ -z "$TEST_TARGET" ]; then
	export TEST_TARGET=test
	export TEST_JOBS=auto
else
	echo "TEST_TARGET: ${TEST_TARGET}"
fi
if [ "$TEST_TARGET" == "test-cov" ]; then
	/local/lcov-prepare
fi

if [ -f "/local/gdb-commands" ]; then
	echo "/local/gdb-commands exists, use it"
	sudo cp /local/gdb-commands /gdb-commands
fi

if [ -f "/local/exec-before-test" ]; then
	echo "/local/exec-before-test exists, use it"
	source /local/exec-before-test
fi

if TEST=${ARG_TEST} UNATTENDED=y make install-dep ${TEST_TARGET}; then
	echo Inside docker: success
else
	EXIT_CODE=$?
	echo "Inside docker: failure, exit code ${EXIT_CODE}"
	echo "Sleep 5 minutes.."
	sleep 300
	cd /

	for CORE in $(find /tmp/vpp* -name core*); do
		echo "FOUND CORE: ${CORE}, invoke GDB"
		BINFILE=$(gdb -c ${CORE} -ex quit | grep 'Core was generated' | awk '{ print $5; }' | sed -e s/\`//g)
		echo ====================================================== DECODE CORE: ${CORE}
		gdb ${BINFILE} ${CORE} -ex 'source -v gdb-commands' -ex quit 
	done
	echo ======= cleaning up the files and archiving =======
	# delete the core files, no need to waste the space on them - we decoded them above
	find /tmp/vpp* -name core* -exec echo DELETE {} \; -exec rm {} \;
	# archive all the failed unittests
	tar czvhf /local/failed-unit-tests.tgz /tmp/vpp-failed-unittests
fi
if [ "$TEST_TARGET" == "test-cov" ]; then
	tar czvhf /local/test-coverage.tgz test/coverage
fi


EXECEND=`date`
echo EXECEND: $EXECEND
exit ${EXIT_CODE}

# while true; do date; sleep 1; done

