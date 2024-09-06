node_arg=$1

if [ -z ${node_arg} ]; then
    cat *.pid | xargs kill -9
    rm *.pid
else
    cat ${node_arg}.pid | xargs kill -9
    rm ${node_arg}.pid
fi
