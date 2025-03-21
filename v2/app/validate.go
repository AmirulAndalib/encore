package app

import (
	"encr.dev/pkg/errors"
	"encr.dev/pkg/paths"
	"encr.dev/v2/internals/parsectx"
	"encr.dev/v2/parser"
	"encr.dev/v2/parser/apis/authhandler"
	"encr.dev/v2/parser/apis/middleware"
	"encr.dev/v2/parser/infra/caches"
	"encr.dev/v2/parser/infra/objects"
	"encr.dev/v2/parser/infra/pubsub"
	"encr.dev/v2/parser/infra/secrets"
	"encr.dev/v2/parser/infra/sqldb"
)

// validate checks that the application is in a valid state across all services and compilation units.
func (d *Desc) validate(pc *parsectx.Context, result *parser.Result) {
	defer pc.Trace("app.validate").Done()

	// Validate the framework
	if fw, ok := d.Framework.Get(); ok {
		d.validateAuthHandlers(pc, fw)
		d.validateAPIs(pc, fw, result)
		d.validateMiddleware(pc, fw)
		d.validateServiceStructs(pc, result)
	}

	// Validate infrastructure
	d.validateCaches(pc, result)
	d.validateConfigs(pc, result)
	d.validateCrons(pc, result)
	d.validateDatabases(pc, result)
	d.validatePubSub(pc, result)
	d.validateObjects(pc, result)

	// Validate all resources are defined within a service
	for _, b := range result.AllBinds() {
		r := result.ResourceForBind(b)
		switch r.(type) {
		case *pubsub.Topic:
			// We allow pubsub topics to be declared outside of service code
			continue
		case *objects.Bucket:
			// We allow buckets to be declared outside of service code
			continue
		case *middleware.Middleware:
			// Middleware is also allowed to be declared outside of service code if it's global (validateMiddleware checks this already)
			continue
		case *authhandler.AuthHandler:
			// AuthHandlers are also allowed to be declared outside of service code as it's shared code between all services
			continue
		case *secrets.Secrets:
			// Secrets are allowed anywhere
			continue
		case *sqldb.Database:
			// Databases are allowed anywhere
			continue
		case *caches.Cluster:
			// Cache clusters are allowed anywhere
			continue

		default:
			_, ok := d.ServiceForPath(b.Package().FSPath)

			// It's permitted to declare resources in test files
			// or in the main pkg in the case of 'encore alpha exec'.
			mainPkgPath := d.BuildInfo.MainPkg.GetOrElse("")
			inTestFile, inMainPkg := false, false
			if file, ok := b.DeclaredIn().Get(); ok {
				if file.TestFile {
					inTestFile = true
				}
				if mainPkgPath != "" && mainPkgPath.LexicallyContains(file.Pkg.ImportPath) {
					inMainPkg = true
				}
			}

			if !ok && !inTestFile && !inMainPkg {
				pc.Errs.Add(errResourceDefinedOutsideOfService.AtGoNode(r))
			}
		}
	}

	// Validate nothing is accessing an et package if it isn't a test file
	etPkg := paths.Pkg("encore.dev/et")
	for _, pkg := range result.AppPackages() {
		for _, file := range pkg.Files {
			if !file.TestFile {
				for importPath, importSpec := range file.Imports {
					if etPkg.LexicallyContains(importPath) {
						pc.Errs.Add(errETPackageUsedOutsideOfTestFile.AtGoNode(importSpec, errors.AsError("imported here")))
					}
				}
			}
		}
	}
}
