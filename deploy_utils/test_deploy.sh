node_arg=$1
rm -rf /tmp/$node_arg

mkdir -p /tmp/$node_arg
mkdir -p /tmp/$node_arg/genesis
mkdir -p /tmp/$node_arg/bin
mkdir -p /tmp/$node_arg/data
mkdir -p /tmp/$node_arg/logs
mkdir -p /tmp/$node_arg/script

cp -r $node_arg/genesis /tmp/$node_arg
cp -r nodes_config.json /tmp/$node_arg/genesis/
cp -r discovery /tmp/$node_arg

cp target/debug/gravity-reth /tmp/$node_arg/bin
cp deploy_utils/start.sh /tmp/$node_arg/script
cp deploy_utils/stop.sh /tmp/$node_arg/script