import pytest
from main import CountryLanguage, QueryCondition,MIN_RATE,MIN_RUNTIME,MIN_YEAR,MAX_RATE,MAX_RUNTIME,MAX_YEAR

def test_valid_json_data():
    """
    测试有效的JSON数据是否能成功创建QueryCondition实例。
    """
    json_string = """
    {
      "title": "Inception",
      "director": ["Christopher Nolan"],
      "start_year": 2010,
      "min_rating": 8.0,
      "country_language": "USA"
    }
    """

    condition = QueryCondition.from_json(json_string)

    # 断言：检查转换后的实例字段是否正确
    assert condition.title == "Inception"
    assert condition.director == ["Christopher Nolan"]
    assert condition.start_year == 2010
    assert condition.min_rating == 8.0
    assert condition.country_language is not None
    assert condition.country_language == CountryLanguage.USA

def test_json_with_optional_fields():
    """
    测试只包含部分可选字段的JSON数据。
    """
    json_string = '{"imdb_id": "tt1375666", "actors": ["Leonardo DiCaprio"]}'

    condition = QueryCondition.from_json(json_string)

    assert condition.imdb_id == "tt1375666"
    assert "Leonardo DiCaprio" in condition.actors
    assert condition.title == ""  # 检查默认值
    assert condition.director == []  # 检查默认值

def test_missing_required_field():
    """
    测试缺少关键查询条件时是否抛出ValueError。
    """
    json_string = "{}"  # 缺少 title/imdb_id/douban_id

    with pytest.raises(ValueError, match="必须包含 'title', 'imdb_id' 或 'douban_id'"):
        QueryCondition.from_json(json_string)

def test_invalid_year_range():
    """
    测试起始年份大于终止年份时是否抛出ValueError。
    """
    json_string = '{"title": "Test", "start_year": 2020, "end_year": 2010}'

    with pytest.raises(ValueError, match="起始年份 2020 不能大于终止年份 2010"):
        QueryCondition.from_json(json_string)

def test_invalid_rating_range():
    """
    测试最低评分大于最高评分时是否抛出ValueError。
    """
    json_string = '{"title": "Test", "min_rating": 9.0, "max_rating": 8.0}'

    with pytest.raises(ValueError, match="最低评分 9.0 不能大于最高评分 8.0"):
        QueryCondition.from_json(json_string)

def test_invalid_json_format():
    """
    测试JSON格式错误时是否抛出ValueError。
    """
    invalid_json = '{"title": "Test", "min_rating": 9.0'  # 缺少大括号

    with pytest.raises(ValueError, match="无法解析 JSON 字符串"):
        QueryCondition.from_json(invalid_json)

def test_invalid_country_language():
    """
    测试无效的国家/语言代码时是否抛出ValueError。
    """
    json_string = '{"title": "Test", "country_language": "invalid_code"}'

    with pytest.raises(ValueError, match="无效的国家/语言代码"):
        QueryCondition.from_json(json_string)

def test_year_range_at_boundary():
    """
    测试年份范围在边界值时是否有效。
    """
    json_string_min = '{"title": "Test", "start_year": %d, "end_year": %d}' % (MIN_YEAR, MIN_YEAR)
    condition_min = QueryCondition.from_json(json_string_min)
    assert condition_min.start_year == MIN_YEAR
    assert condition_min.end_year == MIN_YEAR

    json_string_max = '{"title": "Test", "start_year": %d, "end_year": %d}' % (MAX_YEAR, MAX_YEAR)
    condition_max = QueryCondition.from_json(json_string_max)
    assert condition_max.start_year == MAX_YEAR
    assert condition_max.end_year == MAX_YEAR

def test_year_out_of_range():
    """
    测试年份超出边界时是否抛出ValueError。
    """
    json_string_too_low = '{"title": "Test", "start_year": %d}' % (MIN_YEAR - 1)
    with pytest.raises(ValueError):
        QueryCondition.from_json(json_string_too_low)

    json_string_too_high = '{"title": "Test", "end_year": %d}' % (MAX_YEAR + 1)
    with pytest.raises(ValueError):
        QueryCondition.from_json(json_string_too_high)

def test_rating_range_at_boundary():
    """
    测试评分范围在边界值时是否有效。
    """
    json_string_min = '{"title": "Test", "min_rating": %s, "max_rating": %s}' % (MIN_RATE, MIN_RATE)
    condition_min = QueryCondition.from_json(json_string_min)
    assert condition_min.min_rating == MIN_RATE
    assert condition_min.max_rating == MIN_RATE

    json_string_max = '{"title": "Test", "min_rating": %s, "max_rating": %s}' % (MAX_RATE, MAX_RATE)
    condition_max = QueryCondition.from_json(json_string_max)
    assert condition_max.min_rating == MAX_RATE
    assert condition_max.max_rating == MAX_RATE

def test_rating_out_of_range():
    """
    测试评分超出边界时是否抛出ValueError。
    """
    json_string_too_low = '{"title": "Test", "min_rating": %s}' % (MIN_RATE - 0.1)
    with pytest.raises(ValueError):
        QueryCondition.from_json(json_string_too_low)

    json_string_too_high = '{"title": "Test", "max_rating": %s}' % (MAX_RATE + 0.1)
    with pytest.raises(ValueError):
        QueryCondition.from_json(json_string_too_high)

def test_runtime_range_at_boundary():
    """
    测试片长范围在边界值时是否有效。
    """
    json_string_min = '{"title": "Test", "min_runtime": %d, "max_runtime": %d}' % (MIN_RUNTIME, MIN_RUNTIME)
    condition_min = QueryCondition.from_json(json_string_min)
    assert condition_min.min_runtime == MIN_RUNTIME
    assert condition_min.max_runtime == MIN_RUNTIME

    json_string_max = '{"title": "Test", "min_runtime": %d, "max_runtime": %d}' % (MAX_RUNTIME, MAX_RUNTIME)
    condition_max = QueryCondition.from_json(json_string_max)
    assert condition_max.min_runtime == MAX_RUNTIME
    assert condition_max.max_runtime == MAX_RUNTIME

def test_runtime_out_of_range():
    """
    测试片长超出边界时是否抛出ValueError。
    """
    json_string_too_low = '{"title": "Test", "min_runtime": %d}' % (MIN_RUNTIME - 1)
    with pytest.raises(ValueError):
        QueryCondition.from_json(json_string_too_low)

    json_string_too_high = '{"title": "Test", "max_runtime": %d}' % (MAX_RUNTIME + 1)
    with pytest.raises(ValueError):
        QueryCondition.from_json(json_string_too_high)