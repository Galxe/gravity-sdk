class GravityError(Exception):
    """Base exception class for Gravity E2E framework"""
    pass


class APIError(GravityError):
    """API call error"""
    def __init__(self, message: str, code: int = None):
        self.message = message
        self.code = code
        super().__init__(message)


class ConnectionError(GravityError):
    """Connection error"""
    pass


class NodeError(GravityError):
    """Node-related error"""
    pass