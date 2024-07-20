$@ > /dev/null 1>&1

return_value=$?

if [ $return_value -eq 0 ]; then
  echo "$1: success"
else
  echo "$1: fail"
fi