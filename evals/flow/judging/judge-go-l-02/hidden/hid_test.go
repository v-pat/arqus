package judge
import "testing"
func TestHid(t *testing.T) { p := &Pool{Q: &Queue{}}; p.Q.Push("job2"); if !p.ProcessNext() { t.Fail() } }
