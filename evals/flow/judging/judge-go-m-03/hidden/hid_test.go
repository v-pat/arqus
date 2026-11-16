package judge
import "testing"
func TestHid(t *testing.T) {
    c := Config{Port: 0}
    if c.Validate() == nil { t.Fail() }
}
