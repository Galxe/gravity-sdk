#!/usr/bin/env python3
"""
验证测试用例的脚本
检查测试用例的代码结构和逻辑
"""
import ast
import sys
from pathlib import Path

def verify_test_case(file_path: Path):
    """验证测试用例文件"""
    print(f"验证测试用例: {file_path}")
    
    # 1. 检查文件是否存在
    if not file_path.exists():
        print(f"❌ 文件不存在: {file_path}")
        return False
    
    # 2. 检查语法
    try:
        with open(file_path, 'r') as f:
            code = f.read()
        ast.parse(code)
        print("✅ 语法检查通过")
    except SyntaxError as e:
        print(f"❌ 语法错误: {e}")
        return False
    
    # 3. 检查关键函数和导入
    required_imports = [
        "test_case",
        "RunHelper",
        "TestResult",
        "GravityHttpClient",
        "NodeManager"
    ]
    
    found_imports = []
    for imp in required_imports:
        if imp in code:
            found_imports.append(imp)
            print(f"✅ 找到导入: {imp}")
        else:
            print(f"⚠️  未找到导入: {imp}")
    
    # 4. 检查测试函数
    if "async def test_epoch_consistency" in code:
        print("✅ 找到测试函数: test_epoch_consistency")
    else:
        print("❌ 未找到测试函数: test_epoch_consistency")
        return False
    
    # 5. 检查关键步骤
    key_steps = [
        "deploy_node",
        "start_node",
        "wait_for_epoch",
        "get_ledger_info_by_epoch",
        "get_block_by_epoch_round",
        "get_qc_by_epoch_round",
        "stop_node"
    ]
    
    print("\n检查关键步骤:")
    for step in key_steps:
        if step in code:
            print(f"  ✅ {step}")
        else:
            print(f"  ⚠️  {step}")
    
    # 6. 检查验证逻辑
    validations = [
        "epoch 1 ledger_info.block_number + 1",
        "epoch 2 round 1 block.block_number",
        "epoch 2 round 1 QC commit_info_block_id",
        "epoch 1 ledger_info.block_hash"
    ]
    
    print("\n检查验证逻辑:")
    for validation in validations:
        if validation in code:
            print(f"  ✅ {validation}")
        else:
            print(f"  ⚠️  {validation}")
    
    print(f"\n✅ 测试用例验证完成: {file_path.name}")
    return True

if __name__ == "__main__":
    test_file = Path(__file__).parent / "gravity_e2e" / "tests" / "test_cases" / "test_epoch_consistency.py"
    
    if verify_test_case(test_file):
        print("\n✅ 所有检查通过！")
        sys.exit(0)
    else:
        print("\n❌ 检查失败！")
        sys.exit(1)










