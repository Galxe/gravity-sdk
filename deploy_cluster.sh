# Deploy validator nodes
for i in {1..4}
do
  # 組合節點名稱，例如 "node1", "node2", ...
  NODE_NAME="node$i"

  # 打印出正在執行的命令，方便追蹤
  echo "--- Deploying to ${NODE_NAME} ---"

  # 執行部署命令
  ./deploy_utils/deploy.sh --mode cluster --node "${NODE_NAME}"

  # 檢查上一個命令是否成功，如果不成功就退出
  if [ $? -ne 0 ]; then
    echo "Error: Deployment to ${NODE_NAME} failed."
    exit 1
  fi
done

# Deploy VFN nodes
for i in {1..2}
do
  # 組合節點名稱，例如 "vfn1", "vfn2", ...
  NODE_NAME="vfn$i"

  # 打印出正在執行的命令，方便追蹤
  echo "--- Deploying to ${NODE_NAME} ---"

  # 執行部署命令
  ./deploy_utils/deploy.sh --mode cluster --node "${NODE_NAME}"

  # 檢查上一個命令是否成功，如果不成功就退出
  if [ $? -ne 0 ]; then
    echo "Error: Deployment to ${NODE_NAME} failed."
    exit 1
  fi
done

echo "--- All deployments completed successfully! ---"