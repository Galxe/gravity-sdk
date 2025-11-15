import { Script } from "forge-std/Script.sol";
import { SimpleToken } from "../src/SimpleToken.sol";

contract DeployScript is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);
        
        vm.startBroadcast(deployerPrivateKey);
        
        SimpleToken token = new SimpleToken("TestToken", "TEST", 1000000 * 10**18);
        
        vm.stopBroadcast();
        
        console.log("SimpleToken deployed at:", address(token));
    }
}