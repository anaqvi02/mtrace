package main

import (
	"fmt"
	"net/http"
	"os"
)

func main() {
	fmt.Println("Hello from Go!")
	
	// Create a file to trigger open/write
	f, _ := os.Create("go_test.txt")
	f.WriteString("Go test")
	f.Close()
	
	// Make a network request to trigger socket/connect
	http.Get("http://example.com")
}
