package judge
import "testing"
func TestHid(t *testing.T) { _, ok := FindMax(nil); if ok { t.Fail() } }
