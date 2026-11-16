package judge
import "testing"
func TestHid(t *testing.T) {
    s := &mockSink{}
    c := &Collector{}
    c.AddSink(s)
    c.Emit(Metric{Name: "mem", Value: 5.0})
    if s.count != 1 { t.Fail() }
}
