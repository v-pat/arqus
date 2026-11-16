package judge
import "testing"
func TestHid(t *testing.T) {
    res := CountWords("go go go")
    if res["go"] != 3 { t.Fail() }
}
