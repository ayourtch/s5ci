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
# CACHE_OUTPUT=0 borks on what seems to be big debug CLI outputs
# if TEST=${ARG_TEST} TEST_JOBS=auto UNATTENDED=y CACHE_OUTPUT=0 make install-dep test; then
if [ -z "$TEST_TARGET" ]; then
	export TEST_TARGET=test
	export TEST_JOBS=auto
else
	echo "TEST_TARGET: ${TEST_TARGET}"
fi
if [ "$TEST_TARGET" == "test-cov" ]; then
cat <<__EOP__ | patch -p1
diff --git a/test/Makefile b/test/Makefile
index d8bbf4d5c..77a637c93 100644
--- a/test/Makefile
+++ b/test/Makefile
@@ -242,7 +242,7 @@ wipe-doc:
 cov: wipe-cov reset ext verify-test-dir $(PAPI_INSTALL_DONE)
        @lcov --zerocounters --directory $(VPP_BUILD_DIR)
        @test -z "$(EXTERN_COV_DIR)" || lcov --zerocounters --directory $(EXTERN_COV_DIR)
-       $(call retest-func)
+       -$(call retest-func)
        @mkdir $(BUILD_COV_DIR)
        @lcov --capture --directory $(VPP_BUILD_DIR) --output-file $(BUILD_COV_DIR)/coverage.info
        @test -z "$(EXTERN_COV_DIR)" || lcov --capture --directory $(EXTERN_COV_DIR) --output-file $(BUILD_COV_DIR)/extern-coverage.info
__EOP__
fi


if TEST=${ARG_TEST} UNATTENDED=y make install-dep ${TEST_TARGET}; then
	echo Inside docker: success
else
	EXIT_CODE=$?
	echo Inside docker: failure, exit code ${EXIT_CODE}
	cd /
	for CORE in $(find /tmp/vpp* -name core*); do
		BINFILE=$(gdb -c ${CORE} -ex quit | grep 'Core was generated' | awk '{ print $5; }' | sed -e s/\`//g)
		echo ====================================================== DECODE CORE: ${CORE}
		gdb ${BINFILE} ${CORE} -ex 'source -v gdb-commands' -ex quit 
	done
	echo ======= cleaning up the files and archiving =======
	# delete the core files, no need to waste the space on them - we decoded them above
	find /tmp/vpp* -name core* -exec rm {} \;
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

