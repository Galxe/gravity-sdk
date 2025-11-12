class GravityError(Exception):
    """基础异常类"""
    pass


class APIError(GravityError):
    """API 调用错误"""
    def __init__(self, message: str, code: int = None):
        self.message = message
        self.code = code
        super().__init__(message)


class ConnectionError(GravityError):
    """连接错误"""
    pass


class NodeError(GravityError):
    """节点错误"""
    pass