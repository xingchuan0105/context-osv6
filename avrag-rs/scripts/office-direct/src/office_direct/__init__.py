"""office-direct: Office 直读解析器（docx/xlsx/pptx + 旧二进制 doc/ppt/xls）。"""

from .main import ConverterError, convert, main

__all__ = ["ConverterError", "convert", "main"]
