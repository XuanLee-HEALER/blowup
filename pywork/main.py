from dataclasses import dataclass, field
import json
from typing import List, Optional

# 定义国家/语言枚举，你可以根据需要进行扩充
class CountryLanguage:
    CHINA = "zh"
    USA = "en"
    JAPAN = "ja"

MIN_YEAR = 1900
MAX_YEAR = 3000
MIN_RATE:float = 0
MAX_RATE:float = 10
MIN_RUNTIME = 0
MAX_RUNTIME = 10_000

@dataclass
class QueryCondition:
    """
    一个用于封装所有电影查询条件的数据类
    """
    # 基本查询条件
    # 电影名称
    title: str = field(default_factory=str)
    # IMDb ID
    imdb_id: Optional[str] = None
    # douban ID
    douban_id: Optional[str] = None

    # 电影信息相关
    # 导演名称
    director: List[str] = field(default_factory=list)
    # 演员列表
    actors: List[str] = field(default_factory=list)
    # 摄像师列表
    cinematographer: List[str] = field(default_factory=list)
    # 配乐师列表
    composer: List[str] = field(default_factory=list)

    # 分类与筛选
    # 电影标签
    tags: List[str] = field(default_factory=list)
    # 电影类别
    genres: List[str] = field(default_factory=list)

    # 年份与评分
    # 起始年份（包含）
    start_year: Optional[int] = None
    # 终止年份（包含）
    end_year: Optional[int] = None
    # 最小评分（包含）
    min_rating: Optional[float] = None
    # 最大评分（包含）
    max_rating: Optional[float] = None

    # 片长
    # 最小片长，单位：分钟
    min_runtime: Optional[int] = None
    # 最大片长，单位：分钟
    max_runtime: Optional[int] = None

    # 国家或语言
    country_language: Optional[CountryLanguage] = None

    @classmethod
    def from_json(cls, json_string: str) -> 'QueryCondition':
        """
        从 JSON 字符串创建并校验 QueryCondition 实例
        如果 JSON 无效或校验失败，则抛出 ValueError
        """
        try:
            data = json.loads(json_string)
        except json.JSONDecodeError as e:
            raise ValueError(f"无法解析 JSON 字符串: {e}")

        # 校验：至少有一个关键查询条件
        if not data.get("title") and not data.get("imdb_id") and not data.get("douban_id"):
            raise ValueError("查询条件必须包含 'title', 'imdb_id' 或 'douban_id' 中的至少一个")

        # 校验：年份范围
        start_year = data.get("start_year")
        end_year = data.get("end_year")
        if start_year is not None and start_year < MIN_YEAR:
            raise ValueError(f"起始年份 {start_year} 小于最小年份 {MIN_YEAR}")
        if end_year is not None and end_year > MAX_YEAR:
            raise ValueError(f"终止年份 {end_year} 大于最大年份 {MAX_YEAR}")
        if start_year is not None and end_year is not None and start_year > end_year:
            raise ValueError(f"起始年份 {start_year} 不能大于终止年份 {end_year}")

        # 校验：评分范围
        min_rating = data.get("min_rating")
        max_rating = data.get("max_rating")
        if min_rating is not None and min_rating < MIN_RATE:
            raise ValueError(f"最低评分 {start_year} 小于 {MIN_RATE}")
        if max_rating is not None and max_rating > MAX_RATE:
            raise ValueError(f"最高评分 {end_year} 大于 {MAX_RATE}")
        if min_rating is not None and max_rating is not None and min_rating > max_rating:
            raise ValueError(f"最低评分 {min_rating} 不能大于最高评分 {max_rating}。")

        # 校验：片长范围
        min_runtime = data.get("min_runtime")
        max_runtime = data.get("max_runtime")
        if min_runtime is not None and min_runtime < MIN_RUNTIME:
            raise ValueError(f"最短片长 {min_runtime} 不能短于 {MIN_RUNTIME}。")
        if max_runtime is not None and max_runtime > MAX_RUNTIME:
            raise ValueError(f"最长片长 {max_runtime} 不能长于 {MAX_RUNTIME}。")
        if min_runtime is not None and max_runtime is not None and min_runtime > max_runtime:
            raise ValueError(f"最短片长 {min_runtime} 不能长于最长片长 {max_runtime}。")

        # 转换 CountryLanguage 字符串为枚举类型
        country_language_str = data.get("country_language")
        if country_language_str:
            try:
                data["country_language"] = getattr(CountryLanguage, country_language_str.upper())
            except AttributeError:
                raise ValueError(f"无效的国家/语言代码: '{country_language_str}'。")

        # 使用**kwargs动态创建实例
        return cls(**data)

def main():
    print("Hello from pywork!")


if __name__ == "__main__":
    main()
