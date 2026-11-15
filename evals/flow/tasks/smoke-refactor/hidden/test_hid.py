import converter
res = converter.convert_temp(100.0, "C")
assert res.value == 212.0 and res.unit == "F"

