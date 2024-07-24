export LD_LIBRARY_PATH=/lib:/lib/glibc
# cd /ltp && ./kirk -f ltp -r syscalls
ls ltp/testcases/bin/ | xargs -n 1 -I {} ./test-ltp.sh ltp/testcases/bin/{}