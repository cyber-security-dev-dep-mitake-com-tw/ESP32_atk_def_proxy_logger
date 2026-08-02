package api

import "fmt"

func errNodeNotFound(id string) error {
	return fmt.Errorf("node %q not found or not connected", id)
}
