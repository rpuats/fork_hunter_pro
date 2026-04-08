package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"regexp"
	"strings"
	"sync"
	"time"
)

type Event struct {
	Home    string  `json:"home"`
	Away    string  `json:"away"`
	HomeRaw string  `json:"home_raw"`
	AwayRaw string  `json:"away_raw"`
	O1      float64 `json:"o1"`
	OX      float64 `json:"ox"`
	O2      float64 `json:"o2"`
	BK      string  `json:"bk"`
}

type Fork struct {
	Match         string  `json:"match"`
	ProfitPercent float64 `json:"profit_percent"`
	BKs           string  `json:"bks"`
	Bet1          Bet     `json:"bet1"`
	BetX          Bet     `json:"betX"`
	Bet2          Bet     `json:"bet2"`
}

type Bet struct {
	Outcome string  `json:"outcome"`
	BK      string  `json:"bk"`
	Odd     float64 `json:"odd"`
}

type SharedAPIResponse struct {
	Events        []map[string]interface{} `json:"events"`
	CustomFactors []map[string]interface{} `json:"customFactors"`
}

var reNormalize = regexp.MustCompile(`\(.*?\)`)
var reSpaces = regexp.MustCompile(`\s+`)

func normalize(s string) string {
	s = strings.ToLower(s)
	s = reNormalize.ReplaceAllString(s, "")
	s = strings.TrimSpace(s)
	s = reSpaces.ReplaceAllString(s, " ")
	return s
}

var placeholders = map[string]bool{
	"хозяева": true, "гости": true, "home": true, "away": true,
	"team 1": true, "team 2": true, "команда 1": true, "команда 2": true,
}

func isPlaceholder(home, away string) bool {
	return placeholders[home] || placeholders[away]
}

func fetchJSON(url string, headers map[string]string) ([]byte, error) {
	client := &http.Client{Timeout: 15 * time.Second}
	req, _ := http.NewRequest("GET", url, nil)
	for k, v := range headers {
		req.Header.Set(k, v)
	}
	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	return io.ReadAll(resp.Body)
}

func parseSharedPlatform(url string, headers map[string]string, bkName string) []Event {
	body, err := fetchJSON(url, headers)
	if err != nil {
		fmt.Printf("  ERROR %s: %v\n", bkName, err)
		return nil
	}

	var data SharedAPIResponse
	if err := json.Unmarshal(body, &data); err != nil {
		fmt.Printf("  ERROR %s: JSON parse: %v\n", bkName, err)
		return nil
	}

	factorMap := make(map[string]map[string]float64)
	for _, cf := range data.CustomFactors {
		if eid, ok := cf["e"]; ok {
			eidStr := fmt.Sprintf("%v", eid)
			if factors, ok := cf["factors"].([]interface{}); ok {
				fm := make(map[string]float64)
				for _, f := range factors {
					if fm2, ok := f.(map[string]interface{}); ok {
						if fid, ok := fm2["f"]; ok {
							if v, ok := fm2["v"].(float64); ok {
								fm[fmt.Sprintf("%v", fid)] = v
							}
						}
					}
				}
				factorMap[eidStr] = fm
			}
		}
	}

	var events []Event
	for _, m := range data.Events {
		home, _ := m["team1"].(string)
		away, _ := m["team2"].(string)
		home = strings.TrimSpace(home)
		away = strings.TrimSpace(away)
		if home == "" || away == "" || isPlaceholder(home, away) {
			continue
		}

		eid := fmt.Sprintf("%v", m["id"])
		fs := factorMap[eid]
		if fs == nil {
			continue
		}

		o1 := fs["921"]
		ox := fs["922"]
		o2 := fs["923"]

		if o1 > 1 && o2 > 1 {
			events = append(events, Event{
				Home: normalize(home), Away: normalize(away),
				HomeRaw: home, AwayRaw: away,
				O1: o1, OX: ox, O2: o2, BK: bkName,
			})
		}
	}

	return events
}

func parseLeon() []Event {
	body, err := fetchJSON("https://leon.ru/api-2/betline/events/prematch?ctag=ru-RU",
		map[string]string{"Accept": "application/json", "Referer": "https://leon.ru"})
	if err != nil {
		fmt.Printf("  ERROR Leon: %v\n", err)
		return nil
	}

	var data map[string]interface{}
	if err := json.Unmarshal(body, &data); err != nil {
		fmt.Printf("  ERROR Leon: JSON parse: %v\n", err)
		return nil
	}

	var events []Event
	if evts, ok := data["events"].([]interface{}); ok {
		for _, e := range evts {
			m, _ := e.(map[string]interface{})
			if m == nil {
				continue
			}
			comps, _ := m["competitors"].([]interface{})
			if len(comps) < 2 {
				continue
			}
			c1, _ := comps[0].(map[string]interface{})
			c2, _ := comps[1].(map[string]interface{})
			home, _ := c1["name"].(string)
			away, _ := c2["name"].(string)
			home = strings.TrimSpace(home)
			away = strings.TrimSpace(away)
			if home == "" || away == "" || isPlaceholder(home, away) {
				continue
			}

			var o1, ox, o2 float64
			if markets, ok := m["markets"].([]interface{}); ok {
				for _, mk := range markets {
					market, _ := mk.(map[string]interface{})
					if market == nil {
						continue
					}
					runners, _ := market["runners"].([]interface{})
					if len(runners) == 3 {
						r0, _ := runners[0].(map[string]interface{})
						if r0 != nil && r0["name"] == "1" {
							for _, r := range runners {
								runner, _ := r.(map[string]interface{})
								if runner == nil {
									continue
								}
								n, _ := runner["name"].(string)
								p, _ := runner["price"].(float64)
								if n == "1" {
									o1 = p
								} else if n == "X" {
									ox = p
								} else if n == "2" {
									o2 = p
								}
							}
							break
						}
					}
				}
			}

			if o1 > 1 && o2 > 1 {
				events = append(events, Event{
					Home: normalize(home), Away: normalize(away),
					HomeRaw: home, AwayRaw: away,
					O1: o1, OX: ox, O2: o2, BK: "leon",
				})
			}
		}
	}

	return events
}

func findForks(allEvents map[string][]Event) []Fork {
	var forks []Fork
	bkSlugs := make([]string, 0, len(allEvents))
	for bk := range allEvents {
		bkSlugs = append(bkSlugs, bk)
	}

	for i := 0; i < len(bkSlugs); i++ {
		for j := i + 1; j < len(bkSlugs); j++ {
			bkA := bkSlugs[i]
			bkB := bkSlugs[j]

			homeIndex := make(map[string][]Event)
			for _, e := range allEvents[bkB] {
				homeIndex[e.Home] = append(homeIndex[e.Home], e)
			}

			for _, ea := range allEvents[bkA] {
				candidates := homeIndex[ea.Home]
				for _, eb := range candidates {
					if ea.Away == eb.Away {
						best1 := ea.O1
						if eb.O1 > best1 {
							best1 = eb.O1
						}
						bestX := ea.OX
						if eb.OX > bestX {
							bestX = eb.OX
						}
						best2 := ea.O2
						if eb.O2 > best2 {
							best2 = eb.O2
						}

						if best1 > 1 && bestX > 1 && best2 > 1 {
							margin := 1/best1 + 1/bestX + 1/best2
							if margin < 1 {
								profit := (1 - margin) * 100
								if profit > 1.0 {
									bk1, bkX, bk2 := bkA, bkA, bkA
									if eb.O1 > ea.O1 {
										bk1 = bkB
									}
									if eb.OX > ea.OX {
										bkX = bkB
									}
									if eb.O2 > ea.O2 {
										bk2 = bkB
									}

									forks = append(forks, Fork{
										Match:         fmt.Sprintf("%s vs %s", ea.HomeRaw, ea.AwayRaw),
										ProfitPercent: profit,
										BKs:           fmt.Sprintf("%s vs %s", bkA, bkB),
										Bet1:          Bet{Outcome: "1", BK: bk1, Odd: best1},
										BetX:          Bet{Outcome: "X", BK: bkX, Odd: bestX},
										Bet2:          Bet{Outcome: "2", BK: bk2, Odd: best2},
									})
								}
							}
						}
					}
				}
			}
		}
	}

	return forks
}

func main() {
	fmt.Println("==============================================================")
	fmt.Println("GHOST IMPERIUM - Go Fork Scanner (Ultra-Fast)")
	fmt.Println("==============================================================")

	t0 := time.Now()

	allEvents := make(map[string][]Event)
	var mu sync.Mutex
	var wg sync.WaitGroup

	type task struct {
		fn   func() []Event
		name string
	}

	tasks := []task{
		{parseLeon, "leon"},
		{func() []Event {
			return parseSharedPlatform(
				"https://line-lb01-w.pb06e2-resources.com/events/listBase?lang=ru&scopeMarket=2300",
				map[string]string{"Accept": "application/json", "Referer": "https://pari.ru"}, "pari")
		}, "pari"},
		{func() []Event {
			return parseSharedPlatform(
				"https://line-lb61-w.bk6bba-resources.com/ma/events/listBase?lang=ru&scopeMarket=1600",
				map[string]string{"Accept": "application/json", "Referer": "https://fonbet.ru"}, "fonbet")
		}, "fonbet"},
		{func() []Event {
			return parseSharedPlatform(
				"https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket=3000",
				map[string]string{"Accept": "application/json", "Referer": "https://www.marathonbet.ru"}, "marathon")
		}, "marathon"},
		{func() []Event {
			return parseSharedPlatform(
				"https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket=3000",
				map[string]string{"Accept": "application/json", "Referer": "https://24bet.ru"}, "24bet")
		}, "24bet"},
		{func() []Event {
			return parseSharedPlatform(
				"https://line01.at58f5-resources.com/events/listBase?lang=ru&scopeMarket=501",
				map[string]string{"Accept": "application/json", "Referer": "https://bettery.ru"}, "bettery")
		}, "bettery"},
	}

	fmt.Println("\nFetching events from 6 bookmakers...")
	for _, t := range tasks {
		wg.Add(1)
		go func(t task) {
			defer wg.Done()
			events := t.fn()
			mu.Lock()
			allEvents[t.name] = events
			mu.Unlock()
			fmt.Printf("  OK %s: %d events\n", t.name, len(events))
		}(t)
	}

	wg.Wait()

	total := 0
	for _, evts := range allEvents {
		total += len(evts)
	}
	fmt.Printf("\nTotal: %d events from %d BKs (%.2fs)\n", total, len(allEvents), time.Since(t0).Seconds())

	// Find forks
	fmt.Println("\nFinding forks...")
	t1 := time.Now()
	forks := findForks(allEvents)

	// Sort by profit
	for i := 0; i < len(forks); i++ {
		for j := i + 1; j < len(forks); j++ {
			if forks[j].ProfitPercent > forks[i].ProfitPercent {
				forks[i], forks[j] = forks[j], forks[i]
			}
		}
	}

	fmt.Printf("\nTotal forks found: %d (%.3fs)\n", len(forks), time.Since(t1).Seconds())

	if len(forks) > 0 {
		fmt.Println("\n==============================================================")
		fmt.Println("TOP 10 FORKS:")
		fmt.Println("==============================================================")
		limit := 10
		if len(forks) < limit {
			limit = len(forks)
		}
		for i := 0; i < limit; i++ {
			f := forks[i]
			fmt.Printf("\n#%d %s\n", i+1, f.Match)
			fmt.Printf("   Profit: %.2f%% | BKs: %s\n", f.ProfitPercent, f.BKs)
			fmt.Printf("   -> 1: %s @ %.2f\n", f.Bet1.BK, f.Bet1.Odd)
			fmt.Printf("   -> X: %s @ %.2f\n", f.BetX.BK, f.BetX.Odd)
			fmt.Printf("   -> 2: %s @ %.2f\n", f.Bet2.BK, f.Bet2.Odd)
		}
	}

	// Save to JSON
	output := map[string]interface{}{
		"forks": forks,
		"total": len(forks),
	}
	jsonData, _ := json.MarshalIndent(output, "", "  ")
	os.WriteFile("forks_go_output.json", jsonData, 0644)

	fmt.Printf("\nSaved to forks_go_output.json\n")
	fmt.Println("==============================================================")
}
