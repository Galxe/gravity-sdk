// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

/**
 * @title AdvancedDataTest
 * @dev Contract specifically designed to test advanced eth_call functionality
 * with complex data structures including structs, arrays, and mappings
 */
contract AdvancedDataTest {
    
    // Struct definitions for testing
    struct UserInfo {
        string name;
        uint256 age;
        bool active;
        uint256 balance;
        uint256[] tokens;
        mapping(string => string) metadata;
    }
    
    struct Order {
        uint256 id;
        address trader;
        uint256 amount;
        uint256 price;
        bool isBuy;
        uint256 timestamp;
    }
    
    struct Portfolio {
        uint256 totalValue;
        Asset[] assets;
        uint256 lastUpdated;
    }
    
    struct Asset {
        string symbol;
        uint256 balance;
        uint256 value;
    }
    
    struct TransactionRecord {
        uint256 id;
        address from;
        address to;
        uint256 amount;
        uint256 timestamp;
        string txType;
    }
    
    // State variables
    mapping(address => UserInfo) public users;
    mapping(address => uint256) public tokenBalances;
    mapping(string => Order[]) public orderBooks;
    mapping(address => Portfolio) public portfolios;
    mapping(address => TransactionRecord[]) public transactionHistory;
    mapping(address => mapping(string => uint256)) public customMappings;
    
    uint256 public nextOrderId = 1;
    uint256 public nextTxId = 1;
    address public owner;
    string[] public supportedTokens;
    
    constructor() {
        owner = msg.sender;
        supportedTokens = ["ETH", "USDC", "WBTC", "DAI"];
    }
    
    // User management functions
    function setUserInfo(
        address _user,
        string memory _name,
        uint256 _age,
        bool _active,
        uint256 _balance,
        uint256[] memory _tokens
    ) public {
        require(msg.sender == owner, "Only owner can set user info");
        
        UserInfo storage user = users[_user];
        user.name = _name;
        user.age = _age;
        user.active = _active;
        user.balance = _balance;
        user.tokens = _tokens;
        
        // Set some metadata
        user.metadata["level"] = "gold";
        user.metadata["verified"] = "true";
    }
    
    function getUserInfo(address _user) public view returns (
        string memory name,
        uint256 age,
        bool active,
        uint256 balance,
        uint256[] memory tokens
    ) {
        UserInfo storage user = users[_user];
        return (user.name, user.age, user.active, user.balance, user.tokens);
    }
    
    function getUserDetailedInfo(address _user) public view returns (
        string memory name,
        uint256 age,
        bool active,
        uint256 balance,
        uint256[] memory tokens,
        string memory level,
        string memory verified
    ) {
        UserInfo storage user = users[_user];
        return (
            user.name, 
            user.age, 
            user.active, 
            user.balance, 
            user.tokens,
            user.metadata["level"],
            user.metadata["verified"]
        );
    }
    
    // Token balance functions
    function setTokenBalance(address _user, uint256 _balance) public {
        require(msg.sender == owner, "Only owner can set balance");
        tokenBalances[_user] = _balance;
    }
    
    function getTokenBalance(address _user) public view returns (uint256) {
        return tokenBalances[_user];
    }
    
    function getMultipleTokenBalances(address[] memory _users) public view returns (uint256[] memory) {
        uint256[] memory balances = new uint256[](_users.length);
        for (uint256 i = 0; i < _users.length; i++) {
            balances[i] = tokenBalances[_users[i]];
        }
        return balances;
    }
    
    // Order book functions
    function addOrder(
        string memory _pair,
        address _trader,
        uint256 _amount,
        uint256 _price,
        bool _isBuy
    ) public {
        Order memory newOrder = Order({
            id: nextOrderId,
            trader: _trader,
            amount: _amount,
            price: _price,
            isBuy: _isBuy,
            timestamp: block.timestamp
        });
        
        orderBooks[_pair].push(newOrder);
        nextOrderId++;
    }
    
    function getOrderBook(string memory _pair) public view returns (
        uint256[] memory ids,
        address[] memory traders,
        uint256[] memory amounts,
        uint256[] memory prices,
        bool[] memory isBuys,
        uint256[] memory timestamps
    ) {
        Order[] storage orders = orderBooks[_pair];
        uint256 length = orders.length;
        
        ids = new uint256[](length);
        traders = new address[](length);
        amounts = new uint256[](length);
        prices = new uint256[](length);
        isBuys = new bool[](length);
        timestamps = new uint256[](length);
        
        for (uint256 i = 0; i < length; i++) {
            ids[i] = orders[i].id;
            traders[i] = orders[i].trader;
            amounts[i] = orders[i].amount;
            prices[i] = orders[i].price;
            isBuys[i] = orders[i].isBuy;
            timestamps[i] = orders[i].timestamp;
        }
    }
    
    function getOrdersByTrader(address _trader) public view returns (
        uint256[] memory ids,
        string[] memory pairs,
        uint256[] memory amounts,
        uint256[] memory prices
    ) {
        // Count orders by this trader
        uint256 count = 0;
        string[] memory pairList = new string[](2);
        pairList[0] = "ETH/USDC";
        pairList[1] = "BTC/USDT";
        
        for (uint256 p = 0; p < pairList.length; p++) {
            Order[] storage orders = orderBooks[pairList[p]];
            for (uint256 i = 0; i < orders.length; i++) {
                if (orders[i].trader == _trader) {
                    count++;
                }
            }
        }
        
        // Fill result arrays
        uint256[] memory resultIds = new uint256[](count);
        string[] memory resultPairs = new string[](count);
        uint256[] memory resultAmounts = new uint256[](count);
        uint256[] memory resultPrices = new uint256[](count);
        
        uint256 index = 0;
        for (uint256 p = 0; p < pairList.length; p++) {
            Order[] storage orders = orderBooks[pairList[p]];
            for (uint256 i = 0; i < orders.length; i++) {
                if (orders[i].trader == _trader) {
                    resultIds[index] = orders[i].id;
                    resultPairs[index] = pairList[p];
                    resultAmounts[index] = orders[i].amount;
                    resultPrices[index] = orders[i].price;
                    index++;
                }
            }
        }
        
        return (resultIds, resultPairs, resultAmounts, resultPrices);
    }
    
    // Portfolio functions
    function setPortfolio(
        address _user,
        uint256 _totalValue,
        Asset[] memory _assets
    ) public {
        require(msg.sender == owner, "Only owner can set portfolio");
        
        Portfolio storage portfolio = portfolios[_user];
        portfolio.totalValue = _totalValue;
        portfolio.lastUpdated = block.timestamp;
        
        // Clear existing assets and add new ones
        delete portfolio.assets;
        for (uint256 i = 0; i < _assets.length; i++) {
            portfolio.assets.push(_assets[i]);
        }
    }
    
    function getPortfolio(address _user) public view returns (
        uint256 totalValue,
        string[] memory symbols,
        uint256[] memory balances,
        uint256[] memory values,
        uint256 lastUpdated
    ) {
        Portfolio storage portfolio = portfolios[_user];
        uint256 length = portfolio.assets.length;
        
        symbols = new string[](length);
        balances = new uint256[](length);
        values = new uint256[](length);
        
        for (uint256 i = 0; i < length; i++) {
            symbols[i] = portfolio.assets[i].symbol;
            balances[i] = portfolio.assets[i].balance;
            values[i] = portfolio.assets[i].value;
        }
        
        return (portfolio.totalValue, symbols, balances, values, portfolio.lastUpdated);
    }
    
    // Transaction history functions
    function addTransaction(
        address _from,
        address _to,
        uint256 _amount,
        string memory _txType
    ) public {
        TransactionRecord memory txRecord = TransactionRecord({
            id: nextTxId,
            from: _from,
            to: _to,
            amount: _amount,
            timestamp: block.timestamp,
            txType: _txType
        });
        
        transactionHistory[_from].push(txRecord);
        transactionHistory[_to].push(txRecord);
        nextTxId++;
    }
    
    function getTransactionHistory(address _user, uint256 _limit) public view returns (
        uint256[] memory ids,
        address[] memory fromAddresses,
        address[] memory toAddresses,
        uint256[] memory amounts,
        uint256[] memory timestamps,
        string[] memory txTypes
    ) {
        TransactionRecord[] storage txs = transactionHistory[_user];
        uint256 length = txs.length > _limit ? _limit : txs.length;
        
        ids = new uint256[](length);
        fromAddresses = new address[](length);
        toAddresses = new address[](length);
        amounts = new uint256[](length);
        timestamps = new uint256[](length);
        txTypes = new string[](length);
        
        // Get the most recent transactions (reverse order)
        for (uint256 i = 0; i < length; i++) {
            uint256 txIndex = txs.length - 1 - i;
            ids[i] = txs[txIndex].id;
            fromAddresses[i] = txs[txIndex].from;
            toAddresses[i] = txs[txIndex].to;
            amounts[i] = txs[txIndex].amount;
            timestamps[i] = txs[txIndex].timestamp;
            txTypes[i] = txs[txIndex].txType;
        }
    }
    
    // Complex mapping functions
    function setCustomMapping(
        address _user,
        string memory _key,
        uint256 _value
    ) public {
        require(msg.sender == owner, "Only owner can set custom mapping");
        customMappings[_user][_key] = _value;
    }
    
    function getCustomMapping(address _user, string memory _key) public view returns (uint256) {
        return customMappings[_user][_key];
    }
    
    function getMultipleCustomMappings(
        address _user,
        string[] memory _keys
    ) public view returns (uint256[] memory) {
        uint256[] memory values = new uint256[](_keys.length);
        for (uint256 i = 0; i < _keys.length; i++) {
            values[i] = customMappings[_user][_keys[i]];
        }
        return values;
    }
    
    // Complex nested data functions
    function getUserCompleteData(address _user) public view returns (
        string memory name,
        uint256 age,
        bool active,
        uint256 balance,
        uint256[] memory tokens,
        uint256 tokenBalance,
        uint256 portfolioValue,
        string[] memory assetSymbols,
        uint256[] memory assetBalances,
        uint256 txCount
    ) {
        UserInfo storage user = users[_user];
        Portfolio storage portfolio = portfolios[_user];
        
        string[] memory symbols;
        uint256[] memory balances;
        uint256[] memory values;
        (, symbols, balances, values,) = getPortfolio(_user);
        
        return (
            user.name,
            user.age,
            user.active,
            user.balance,
            user.tokens,
            tokenBalances[_user],
            portfolio.totalValue,
            symbols,
            balances,
            transactionHistory[_user].length
        );
    }
    
    // Array manipulation functions
    function getSupportedTokens() public view returns (string[] memory) {
        return supportedTokens;
    }
    
    function getNumericArrays() public pure returns (
        uint256[] memory,
        int256[] memory,
        bool[] memory
    ) {
        uint256[] memory uintArray = new uint256[](5);
        int256[] memory intArray = new int256[](5);
        bool[] memory boolArray = new bool[](5);
        
        for (uint256 i = 0; i < 5; i++) {
            uintArray[i] = i * 10;
            intArray[i] = int256(i) - 2;
            boolArray[i] = (i % 2 == 0);
        }
        
        return (uintArray, intArray, boolArray);
    }
    
    // Utility functions for testing
    function getContractStats() public view returns (
        uint256 totalUsers,
        uint256 totalOrders,
        uint256 totalTransactions,
        uint256 supportedTokensCount
    ) {
        // These are simplified counts - in practice you'd maintain counters
        return (100, 50, 200, supportedTokens.length);
    }
}