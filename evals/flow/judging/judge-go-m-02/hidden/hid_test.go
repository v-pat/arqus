package judge
import "testing"
func TestHid(t *testing.T) { if !MatchRoute("*", "/test") { t.Fail() } }
