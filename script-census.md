# PrivetDr script census — structural bytecode walk

Linear operand-correct walk (`census_stream`) of every unique function/state code stream on every PrivetDr actor's class chain.

## Streams

| stream | kind | bytes | result |
|---|---|---:|---|
| `Engine.LevelInfo.PreBeginPlay` | function | 22 | ok |
| `Engine.LevelInfo.ServerTravel` | function | 65 | ok |
| `Engine.ZoneInfo.PreBeginPlay` | function | 17 | ok |
| `Engine.ZoneInfo.ActorLeaving` | function | 85 | ok |
| `Engine.ZoneInfo.ActorEntered` | function | 273 | ok |
| `Engine.ZoneInfo.Trigger` | function | 17 | ok |
| `Engine.ZoneInfo.ZoneActors` | function | 6 | ok |
| `Engine.ZoneInfo.LinkToSkybox` | function | 74 | ok |
| `Engine.Actor.Trigger` | function | 2 | ok |
| `Engine.Actor.PostBeginPlay` | function | 2 | ok |
| `Engine.Actor.Timer` | function | 2 | ok |
| `Engine.Actor.Tick` | function | 2 | ok |
| `Engine.Actor.PreBeginPlay` | function | 76 | ok |
| `Engine.Actor.BeginPlay` | function | 2 | ok |
| `Engine.Actor.Destroyed` | function | 2 | ok |
| `Engine.Actor.UnTrigger` | function | 2 | ok |
| `Engine.Actor.AnimEnd` | function | 2 | ok |
| `Engine.Actor.Touch` | function | 2 | ok |
| `Engine.Actor.ZoneChange` | function | 2 | ok |
| `Engine.Actor.SpecialHandling` | function | 2 | ok |
| `Engine.Actor.TakeDamage` | function | 2 | ok |
| `Engine.Actor.TravelPostAccept` | function | 2 | ok |
| `Engine.Actor.CutCommand` | function | 416 | ok |
| `Engine.Actor.KilledBy` | function | 2 | ok |
| `Engine.Actor.EncroachingOn` | function | 2 | ok |
| `Engine.Actor.Landed` | function | 2 | ok |
| `Engine.Actor.GetHumanName` | function | 10 | ok |
| `Engine.Actor.TravelPreAccept` | function | 2 | ok |
| `Engine.Actor.RenderOverlays` | function | 2 | ok |
| `Engine.Actor.GetNextIntDesc` | function | 12 | ok |
| `Engine.Actor.GetSoundDuration` | function | 3 | ok |
| `Engine.Actor.CutCue` | function | 37 | ok |
| `Engine.Actor.PlayMusic` | function | 6 | ok |
| `Engine.Actor.PlayOwnedSound` | function | 24 | ok |
| `Engine.Actor.StopAllMusic` | function | 3 | ok |
| `Engine.Actor.BaseChange` | function | 2 | ok |
| `Engine.Actor.Bump` | function | 2 | ok |
| `Engine.Actor.EndEvent` | function | 2 | ok |
| `Engine.Actor.UnTouch` | function | 2 | ok |
| `Engine.Actor.BeginEvent` | function | 2 | ok |
| `Engine.Actor.HitWall` | function | 2 | ok |
| `Engine.Actor.EncroachedBy` | function | 2 | ok |
| `Engine.Actor.Falling` | function | 2 | ok |
| `Engine.Actor.Attach` | function | 2 | ok |
| `Engine.Actor.FinishedInterpolation` | function | 2 | ok |
| `Engine.Actor.FellOutOfWorld` | function | 10 | ok |
| `Engine.Actor.StopMusic` | function | 6 | ok |
| `Engine.Actor.ConsoleCommand` | function | 3 | ok |
| `Engine.Actor.OnResolveGameState` | function | 2 | ok |
| `Engine.Actor.SetDefaultDisplayProperties` | function | 34 | ok |
| `Engine.Actor.PostNetBeginPlay` | function | 2 | ok |
| `Engine.Actor.SetDisplayProperties` | function | 34 | ok |
| `Engine.Actor.BecomeViewTarget` | function | 2 | ok |
| `Engine.Actor.cm` | function | 21 | ok |
| `Engine.Actor.ComputeTrajectoryByTime` | function | 120 | ok |
| `Engine.Actor.OnLumosOff` | function | 2 | ok |
| `Engine.Actor.OnLumosOn` | function | 2 | ok |
| `Engine.Actor.LoadGameSaveInfo` | function | 6 | ok |
| `Engine.Actor.SaveGameSaveInfo` | function | 6 | ok |
| `Engine.Actor.LoadObjectAsFile` | function | 6 | ok |
| `Engine.Actor.SaveObjectAsFile` | function | 6 | ok |
| `Engine.Actor.CreateTextureFromBMP` | function | 6 | ok |
| `Engine.Actor.CreateTextureFromScreenShot` | function | 3 | ok |
| `Engine.Actor.StartInterpolation` | function | 61 | ok |
| `Engine.Actor.TriggerDisable` | function | 4 | ok |
| `Engine.Actor.UntriggerEvent` | function | 58 | ok |
| `Engine.Actor.TriggerEvent` | function | 58 | ok |
| `Engine.Actor.GetItemName` | function | 68 | ok |
| `Engine.Actor.RenderTexture` | function | 2 | ok |
| `Engine.Actor.HurtRadiusCallBack` | function | 4 | ok |
| `Engine.Actor.HurtRadius` | function | 226 | ok |
| `Engine.Actor.SetInitialState` | function | 31 | ok |
| `Engine.Actor.BroadcastLocalizedMessage` | function | 209 | ok |
| `Engine.Actor.BroadcastMessage` | function | 234 | ok |
| `Engine.Actor.Multiply_ColorFloat` | function | 6 | ok |
| `Engine.Actor.Add_ColorColor` | function | 6 | ok |
| `Engine.Actor.Multiply_FloatColor` | function | 6 | ok |
| `Engine.Actor.Subtract_ColorColor` | function | 6 | ok |
| `Engine.Actor.VisibleCollidingActors` | function | 15 | ok |
| `Engine.Actor.VisibleActors` | function | 12 | ok |
| `Engine.Actor.RadiusActors` | function | 12 | ok |
| `Engine.Actor.TraceActors` | function | 21 | ok |
| `Engine.Actor.TouchingActors` | function | 6 | ok |
| `Engine.Actor.BasedActors` | function | 6 | ok |
| `Engine.Actor.ChildActors` | function | 6 | ok |
| `Engine.Actor.AllActors` | function | 9 | ok |
| `Engine.Actor.MoveCacheEntry` | function | 6 | ok |
| `Engine.Actor.GetCacheEntry` | function | 9 | ok |
| `Engine.Actor.GetNextInt` | function | 6 | ok |
| `Engine.Actor.GetNextSkin` | function | 15 | ok |
| `Engine.Actor.GetMapName` | function | 9 | ok |
| `Engine.Actor.PostTeleport` | function | 2 | ok |
| `Engine.Actor.PreTeleport` | function | 2 | ok |
| `Engine.Actor.MakeNoise` | function | 3 | ok |
| `Engine.Actor.StopSound` | function | 9 | ok |
| `Engine.Actor.ModifySound` | function | 12 | ok |
| `Engine.Actor.DemoPlaySound` | function | 24 | ok |
| `Engine.Actor.PlaySound` | function | 24 | ok |
| `Engine.Actor.SetTimer` | function | 6 | ok |
| `Engine.Actor.Spawn` | function | 15 | ok |
| `Engine.Actor.TraceTexture` | function | 12 | ok |
| `Engine.Actor.FastTrace` | function | 6 | ok |
| `Engine.Actor.Trace` | function | 18 | ok |
| `Engine.Actor.targeted` | function | 2 | ok |
| `Engine.Actor.EndedRotation` | function | 2 | ok |
| `Engine.Actor.KillCredit` | function | 2 | ok |
| `Engine.Actor.Detach` | function | 2 | ok |
| `Engine.Actor.PostTouch` | function | 2 | ok |
| `Engine.Actor.LostChild` | function | 2 | ok |
| `Engine.Actor.GainedChild` | function | 2 | ok |
| `Engine.Actor.Expired` | function | 2 | ok |
| `Engine.Actor.Spawned` | function | 2 | ok |
| `Engine.Actor.AttachToOwner` | function | 27 | ok |
| `Engine.Actor.SetPhysics` | function | 3 | ok |
| `Engine.Actor.LinkSkelAnim` | function | 3 | ok |
| `Engine.Actor.BoneRot` | function | 3 | ok |
| `Engine.Actor.BonePos` | function | 3 | ok |
| `Engine.Actor.BoneName` | function | 3 | ok |
| `Engine.Actor.BoneNumber` | function | 3 | ok |
| `Engine.Actor.CreateAnimChannel` | function | 15 | ok |
| `Engine.Actor.HasAnim` | function | 3 | ok |
| `Engine.Actor.FinishAnim` | function | 3 | ok |
| `Engine.Actor.GetAnimGroup` | function | 3 | ok |
| `Engine.Actor.IsAnimating` | function | 3 | ok |
| `Engine.Actor.TweenAnim` | function | 6 | ok |
| `Engine.Actor.LoopAnim` | function | 18 | ok |
| `Engine.Actor.PlayAnim` | function | 15 | ok |
| `Engine.Actor.SetOwner` | function | 3 | ok |
| `Engine.Actor.SetBase` | function | 3 | ok |
| `Engine.Actor.AutonomousPhysics` | function | 3 | ok |
| `Engine.Actor.MoveSmooth` | function | 3 | ok |
| `Engine.Actor.SetRotation` | function | 3 | ok |
| `Engine.Actor.SetLocation` | function | 3 | ok |
| `Engine.Actor.Move` | function | 3 | ok |
| `Engine.Actor.GetWorldCollisionBox` | function | 3 | ok |
| `Engine.Actor.SetCollisionSize` | function | 9 | ok |
| `Engine.Actor.SetCollision` | function | 9 | ok |
| `Engine.Actor.Sleep` | function | 3 | ok |
| `Engine.Actor.Error` | function | 3 | ok |
| `Engine.Actor.ParseDelimitedString` | function | 144 | ok |
| `Engine.Actor.CutQuestion` | function | 60 | ok |
| `Engine.Actor.FancySpawn` | function | 445 | ok |
| `Engine.Actor.SetLocation2` | function | 63 | ok |
| `Engine.Actor.HandleFacialExpression` | function | 114 | ok |
| `Engine.Actor.CutCommand_Say` | function | 396 | ok |
| `Engine.Actor.DeliverLocalizedDialog` | function | 223 | ok |
| `Engine.Actor.StringToAnimName` | function | 23 | ok |
| `Core.Object.Loge` | function | 3 | ok |
| `Core.Object.SubtractEqual_FloatFloat` | function | 6 | ok |
| `Core.Object.DivideEqual_FloatFloat` | function | 6 | ok |
| `Core.Object.Sqrt` | function | 3 | ok |
| `Core.Object.Exp` | function | 3 | ok |
| `Core.Object.AddAdd_Int` | function | 3 | ok |
| `Core.Object.FMax` | function | 6 | ok |
| `Core.Object.Atan` | function | 3 | ok |
| `Core.Object.Cos` | function | 3 | ok |
| `Core.Object.FClamp` | function | 9 | ok |
| `Core.Object.Tan` | function | 3 | ok |
| `Core.Object.Sin` | function | 3 | ok |
| `Core.Object.AddAdd_PreInt` | function | 3 | ok |
| `Core.Object.Abs` | function | 3 | ok |
| `Core.Object.Max` | function | 6 | ok |
| `Core.Object.AddAdd_PreByte` | function | 3 | ok |
| `Core.Object.AddEqual_FloatFloat` | function | 6 | ok |
| `Core.Object.Multiply_FloatVector` | function | 5 | ok |
| `Core.Object.Multiply_VectorVector` | function | 6 | ok |
| `Core.Object.MultiplyEqual_FloatFloat` | function | 6 | ok |
| `Core.Object.Divide_VectorFloat` | function | 6 | ok |
| `Core.Object.ComplementEqual_FloatFloat` | function | 6 | ok |
| `Core.Object.NotEqual_FloatFloat` | function | 6 | ok |
| `Core.Object.Subtract_VectorVector` | function | 6 | ok |
| `Core.Object.GreaterEqual_FloatFloat` | function | 6 | ok |
| `Core.Object.EqualEqual_FloatFloat` | function | 6 | ok |
| `Core.Object.GreaterGreater_VectorRotator` | function | 6 | ok |
| `Core.Object.Greater_FloatFloat` | function | 6 | ok |
| `Core.Object.LessEqual_FloatFloat` | function | 6 | ok |
| `Core.Object.EqualEqual_VectorVector` | function | 6 | ok |
| `Core.Object.NotEqual_VectorVector` | function | 6 | ok |
| `Core.Object.Subtract_FloatFloat` | function | 6 | ok |
| `Core.Object.Less_FloatFloat` | function | 6 | ok |
| `Core.Object.Dot_VectorVector` | function | 6 | ok |
| `Core.Object.Cross_VectorVector` | function | 6 | ok |
| `Core.Object.Percent_FloatFloat` | function | 6 | ok |
| `Core.Object.Add_FloatFloat` | function | 6 | ok |
| `Core.Object.MultiplyEqual_VectorFloat` | function | 6 | ok |
| `Core.Object.MultiplyEqual_VectorVector` | function | 6 | ok |
| `Core.Object.Multiply_FloatFloat` | function | 6 | ok |
| `Core.Object.Divide_FloatFloat` | function | 6 | ok |
| `Core.Object.DivideEqual_VectorFloat` | function | 6 | ok |
| `Core.Object.Subtract_PreFloat` | function | 3 | ok |
| `Core.Object.AddEqual_VectorVector` | function | 6 | ok |
| `Core.Object.MultiplyMultiply_FloatFloat` | function | 6 | ok |
| `Core.Object.SubtractEqual_VectorVector` | function | 6 | ok |
| `Core.Object.Clamp` | function | 9 | ok |
| `Core.Object.Vec` | function | 29 | ok |
| `Core.Object.Square` | function | 3 | ok |
| `Core.Object.VSize` | function | 3 | ok |
| `Core.Object.Min` | function | 6 | ok |
| `Core.Object.Normal` | function | 3 | ok |
| `Core.Object.Rand` | function | 3 | ok |
| `Core.Object.Invert` | function | 9 | ok |
| `Core.Object.MirrorVectorByNormal` | function | 6 | ok |
| `Core.Object.SubtractSubtract_Int` | function | 3 | ok |
| `Core.Object.VSize2D` | function | 17 | ok |
| `Core.Object.PlaneF` | function | 37 | ok |
| `Core.Object.SubtractSubtract_PreInt` | function | 3 | ok |
| `Core.Object.FMin` | function | 6 | ok |
| `Core.Object.PlaneV` | function | 40 | ok |
| `Core.Object.PlaneC` | function | 33 | ok |
| `Core.Object.Add_PlanePlane` | function | 46 | ok |
| `Core.Object.AddEqual_PlanePlane` | function | 45 | ok |
| `Core.Object.Subtract_PlanePlane` | function | 46 | ok |
| `Core.Object.SubtractEqual_PlanePlane` | function | 45 | ok |
| `Core.Object.Multiply_PlaneFloat` | function | 38 | ok |
| `Core.Object.MultiplyEqual_PlaneFloat` | function | 37 | ok |
| `Core.Object.EqualEqual_RotatorRotator` | function | 6 | ok |
| `Core.Object.SubtractEqual_IntInt` | function | 6 | ok |
| `Core.Object.NotEqual_RotatorRotator` | function | 6 | ok |
| `Core.Object.AddEqual_IntInt` | function | 6 | ok |
| `Core.Object.Multiply_RotatorFloat` | function | 6 | ok |
| `Core.Object.DivideEqual_IntFloat` | function | 6 | ok |
| `Core.Object.Multiply_FloatRotator` | function | 6 | ok |
| `Core.Object.MultiplyEqual_IntFloat` | function | 6 | ok |
| `Core.Object.Divide_RotatorFloat` | function | 6 | ok |
| `Core.Object.Or_IntInt` | function | 6 | ok |
| `Core.Object.MultiplyEqual_RotatorFloat` | function | 6 | ok |
| `Core.Object.Xor_IntInt` | function | 6 | ok |
| `Core.Object.DivideEqual_RotatorFloat` | function | 6 | ok |
| `Core.Object.And_IntInt` | function | 6 | ok |
| `Core.Object.Add_RotatorRotator` | function | 6 | ok |
| `Core.Object.NotEqual_IntInt` | function | 6 | ok |
| `Core.Object.Subtract_RotatorRotator` | function | 6 | ok |
| `Core.Object.EqualEqual_IntInt` | function | 6 | ok |
| `Core.Object.AddEqual_RotatorRotator` | function | 6 | ok |
| `Core.Object.GreaterEqual_IntInt` | function | 6 | ok |
| `Core.Object.SubtractEqual_RotatorRotator` | function | 6 | ok |
| `Core.Object.LessEqual_IntInt` | function | 6 | ok |
| `Core.Object.Subtract_PreInt` | function | 3 | ok |
| `Core.Object.GetAxes` | function | 12 | ok |
| `Core.Object.Greater_IntInt` | function | 6 | ok |
| `Core.Object.Less_IntInt` | function | 6 | ok |
| `Core.Object.GetUnAxes` | function | 12 | ok |
| `Core.Object.GreaterGreaterGreater_IntInt` | function | 6 | ok |
| `Core.Object.GreaterGreater_IntInt` | function | 6 | ok |
| `Core.Object.RotRand` | function | 3 | ok |
| `Core.Object.OrthoRotation` | function | 9 | ok |
| `Core.Object.LessLess_IntInt` | function | 6 | ok |
| `Core.Object.Lerp` | function | 9 | ok |
| `Core.Object.Subtract_IntInt` | function | 6 | ok |
| `Core.Object.Normalize` | function | 3 | ok |
| `Core.Object.Concat_StrStr` | function | 6 | ok |
| `Core.Object.Add_IntInt` | function | 6 | ok |
| `Core.Object.At_StrStr` | function | 6 | ok |
| `Core.Object.Divide_IntInt` | function | 6 | ok |
| `Core.Object.Less_StrStr` | function | 6 | ok |
| `Core.Object.Multiply_IntInt` | function | 6 | ok |
| `Core.Object.Greater_StrStr` | function | 6 | ok |
| `Core.Object.LessEqual_StrStr` | function | 6 | ok |
| `Core.Object.Complement_PreInt` | function | 3 | ok |
| `Core.Object.GreaterEqual_StrStr` | function | 6 | ok |
| `Core.Object.SubtractSubtract_PreByte` | function | 3 | ok |
| `Core.Object.AddAdd_Byte` | function | 3 | ok |
| `Core.Object.EqualEqual_StrStr` | function | 6 | ok |
| `Core.Object.SubtractSubtract_Byte` | function | 3 | ok |
| `Core.Object.NotEqual_StrStr` | function | 6 | ok |
| `Core.Object.OrOr_BoolBool` | function | 6 | ok |
| `Core.Object.ComplementEqual_StrStr` | function | 6 | ok |
| `Core.Object.MultiplyEqual_ByteByte` | function | 6 | ok |
| `Core.Object.Len` | function | 3 | ok |
| `Core.Object.InStr` | function | 6 | ok |
| `Core.Object.SubtractEqual_ByteByte` | function | 6 | ok |
| `Core.Object.Mid` | function | 9 | ok |
| `Core.Object.AddEqual_ByteByte` | function | 6 | ok |
| `Core.Object.DivideEqual_ByteByte` | function | 6 | ok |
| `Core.Object.Left` | function | 6 | ok |
| `Core.Object.Right` | function | 6 | ok |
| `Core.Object.Caps` | function | 3 | ok |
| `Core.Object.Smerp` | function | 7 | ok |
| `Core.Object.Chr` | function | 3 | ok |
| `Core.Object.XorXor_BoolBool` | function | 6 | ok |
| `Core.Object.Asc` | function | 3 | ok |
| `Core.Object.EqualEqual_ObjectObject` | function | 6 | ok |
| `Core.Object.AndAnd_BoolBool` | function | 6 | ok |
| `Core.Object.NotEqual_ObjectObject` | function | 6 | ok |
| `Core.Object.NotEqual_BoolBool` | function | 6 | ok |
| `Core.Object.EqualEqual_NameName` | function | 6 | ok |
| `Core.Object.EqualEqual_BoolBool` | function | 6 | ok |
| `Core.Object.NotEqual_NameName` | function | 6 | ok |
| `Core.Object.Not_PreBool` | function | 3 | ok |
| `Core.Object.Log` | function | 5 | ok |
| `Core.Object.Warn` | function | 2 | ok |
| `Core.Object.Localize` | function | 6 | ok |
| `Core.Object.GotoState` | function | 5 | ok |
| `Core.Object.IsInState` | function | 2 | ok |
| `Core.Object.ClassIsChildOf` | function | 4 | ok |
| `Core.Object.Subtract_PreVector` | function | 2 | ok |
| `Core.Object.IsA` | function | 3 | ok |
| `Core.Object.Enable` | function | 2 | ok |
| `Core.Object.Disable` | function | 2 | ok |
| `Core.Object.Multiply_VectorFloat` | function | 4 | ok |
| `Core.Object.GetPropertyText` | function | 2 | ok |
| `Core.Object.SetPropertyText` | function | 5 | ok |
| `Core.Object.GetEnum` | function | 4 | ok |
| `Core.Object.DynamicLoadObject` | function | 7 | ok |
| `Core.Object.RandRange` | function | 4 | ok |
| `Core.Object.Add_VectorVector` | function | 6 | ok |
| `Core.Object.BeginState` | function | 2 | ok |
| `Core.Object.EndState` | function | 2 | ok |
| `Core.Object.LessLess_VectorRotator` | function | 6 | ok |
| `Engine.NavigationPoint.PlayTeleportEffect` | function | 26 | ok |
| `Engine.NavigationPoint.Accept` | function | 89 | ok |
| `Engine.NavigationPoint.describeSpec` | function | 15 | ok |
| `Engine.NavigationPoint.SpecialCost` | function | 2 | ok |
| `Engine.PatrolPoint.PreBeginPlay` | function | 165 | ok |
| `Engine.PatrolPoint.InitNextPatrolPoint` | function | 257 | ok |
| `Engine.PatrolPoint.CalcFraySplineTangent` | function | 227 | ok |
| `Engine.PatrolPoint.GetTanLenIn` | function | 31 | ok |
| `Engine.PatrolPoint.GetTanLenOut` | function | 6 | ok |
| `Engine.InterpolationPoint.BeginPlay` | function | 955 | ok |
| `Engine.InterpolationPoint.PlusDir` | function | 76 | ok |
| `Engine.InterpolationPoint.DetermineNextDestination` | function | 376 | ok |
| `Engine.InterpolationPoint.InterpolateEnd` | function | 270 | ok |
| `HGame.HiddenHPawn.PreBeginPlay` | function | 42 | ok |
| `HGame.HPawn.PostBeginPlay` | function | 39 | ok |
| `HGame.HPawn.Tick` | function | 78 | ok |
| `HGame.HPawn.HandleSpellRictusempra` | function | 4 | ok |
| `HGame.HPawn.Destroyed` | function | 76 | ok |
| `HGame.HPawn.CutCommand` | function | 698 | ok |
| `HGame.HPawn.PlayerCutCapture` | function | 2 | ok |
| `HGame.HPawn.HandleSpellDiffindo` | function | 4 | ok |
| `HGame.HPawn.PlayerCutRelease` | function | 2 | ok |
| `HGame.HPawn.stateIdle` | state | 1 | ok |
| `HGame.HPawn.PreBeginPlay` | function | 111 | ok |
| `HGame.HPawn.HandleSpellFlipendo` | function | 4 | ok |
| `HGame.HPawn.TakeDamage` | function | 27 | ok |
| `HGame.HPawn.HandleSpellSkurge` | function | 4 | ok |
| `HGame.HPawn.OnEvent` | function | 13 | ok |
| `HGame.HPawn.HandleSpellAlohomora` | function | 4 | ok |
| `HGame.HPawn.OnResolveGameState` | function | 23 | ok |
| `HGame.HPawn.ColObjTouch` | function | 2 | ok |
| `HGame.HPawn.ShouldPlayIdleOnRelease` | function | 4 | ok |
| `HGame.HPawn.ThrownLanded` | function | 2 | ok |
| `HGame.HPawn.HandleSpellEcto` | function | 4 | ok |
| `HGame.HPawn.HandleSpellLumos` | function | 4 | ok |
| `HGame.HPawn.HandleSpellSpongify` | function | 4 | ok |
| `HGame.HPawn.stateBeingThrown` | state | 23 | ok |
| `HGame.HPawn.stateBeingThrown.Landed` | state-function | 9 | ok |
| `HGame.HPawn.stateBeingThrown.HitWall` | state-function | 34 | ok |
| `HGame.HPawn.stateBeingThrown.Bump` | state-function | 31 | ok |
| `HGame.HPawn.stateBeingThrown.Touch` | state-function | 32 | ok |
| `HGame.HPawn.PawnHearHarryNoise` | function | 2 | ok |
| `HGame.HPawn.patrol` | state | 608 | ok |
| `HGame.HPawn.patrol.Tick` | state-function | 33 | ok |
| `HGame.HPawn.patrol.startup` | state-function | 194 | ok |
| `HGame.HPawn.patrol.EndState` | state-function | 23 | ok |
| `HGame.HPawn.patrol.ShouldPlayIdleOnRelease` | state-function | 4 | ok |
| `HGame.HPawn._PostPawnAtPatrolPoint` | function | 80 | ok |
| `HGame.HPawn.PostPawnAtPatrolPoint` | function | 2 | ok |
| `HGame.HPawn.HandleSpellDuelRictusempra` | function | 4 | ok |
| `HGame.HPawn.HandleSpellDuelMimblewimble` | function | 4 | ok |
| `HGame.HPawn.stateDestroy` | state | 42 | ok |
| `HGame.HPawn.HitByThrownObject` | function | 531 | ok |
| `HGame.HPawn.HandleSpellDuelExpelliarmus` | function | 4 | ok |
| `HGame.HPawn.PawnCantStandOnMe` | function | 7 | ok |
| `HGame.HPawn.EaseBetween2` | function | 215 | ok |
| `HGame.HPawn.EaseFrom` | function | 93 | ok |
| `HGame.HPawn.EaseBetween` | function | 112 | ok |
| `HGame.HPawn.SetDespawnFlag` | function | 8 | ok |
| `HGame.HPawn.EaseTo` | function | 115 | ok |
| `HGame.HPawn.EaseFunction` | function | 72 | ok |
| `HGame.HPawn.CutCommand_MatchRot` | function | 297 | ok |
| `HGame.HPawn.LocationSameZ` | function | 58 | ok |
| `HGame.HPawn.PlayDialogLoud` | function | 36 | ok |
| `HGame.HPawn.CutCommand_LeadActor` | function | 544 | ok |
| `HGame.HPawn.PlayDialog` | function | 211 | ok |
| `HGame.HPawn.CutCommand_SetOnPatrolPointPath` | function | 241 | ok |
| `HGame.HPawn.CutCommand_FlyTo` | function | 986 | ok |
| `HGame.HPawn.CutCommand_CastSpell` | function | 555 | ok |
| `HGame.HPawn.CutCommand_FadeTo` | function | 238 | ok |
| `HGame.HPawn.OnSpellHit` | function | 58 | ok |
| `HGame.HPawn.SpawnSpellEx` | function | 39 | ok |
| `HGame.HPawn.SpawnSpell` | function | 178 | ok |
| `HGame.HPawn.LeadActor_ShouldMoveToNextPatrolPoint` | function | 118 | ok |
| `HGame.HPawn.stateLeadingActorPause` | state | 174 | ok |
| `HGame.HPawn.stateLeadingActorPause.AnimEnd` | state-function | 25 | ok |
| `HGame.HPawn.stateLeadingActorPause.ShouldPlayIdleOnRelease` | state-function | 4 | ok |
| `HGame.HPawn.stateLeadingActorPause.Tick` | state-function | 24 | ok |
| `HGame.HPawn.OnFlyToDone` | function | 72 | ok |
| `HGame.HPawn.DoFlyTo_Actor` | function | 94 | ok |
| `HGame.HPawn.PawnAtPatrolPoint` | function | 26 | ok |
| `HGame.HPawn._PawnAtPatrolPoint` | function | 41 | ok |
| `HGame.HPawn.PawnAtDestination` | function | 9 | ok |
| `HGame.HPawn.PawnAtStation` | function | 2 | ok |
| `HGame.HPawn.MoveTo_FraySpline` | function | 565 | ok |
| `HGame.HPawn.NavigateToPathNode` | function | 297 | ok |
| `HGame.HPawn.DestroyControllers` | function | 26 | ok |
| `HGame.HPawn.CreateAttachedParticleFX` | function | 179 | ok |
| `HGame.HPawn.CutCommand_Template` | function | 203 | ok |
| `HGame.HPawn.HandleSpellIncantationSound` | function | 113 | ok |
| `HGame.HPawn.stateLeadingActor` | state | 1 | ok |
| `HGame.HPawn.stateLeadingActor.ShouldPlayIdleOnRelease` | state-function | 4 | ok |
| `HGame.HPawn.stateLeadingActor.PostPawnAtPatrolPoint` | state-function | 64 | ok |
| `HGame.HPawn.LeadActor` | function | 48 | ok |
| `HGame.HPawn.DoFlyToSetup` | function | 202 | ok |
| `HGame.HPawn.DoFlyTo` | function | 47 | ok |
| `HGame.HPawn.FindClosestNavigationPoint` | function | 76 | ok |
| `HGame.HPawn.FindClosestPatrolPoint` | function | 132 | ok |
| `HGame.HPawn.patrolPlayWalkAnim` | function | 14 | ok |
| `HGame.HPawn.patrolPlayRunAnim` | function | 14 | ok |
| `HGame.HPawn.MoveTo_Fray` | function | 297 | ok |
| `HGame.HPawn.statePatrolPointPause` | state | 127 | ok |
| `HGame.HPawn.statePatrolPointPause.ShouldPlayIdleOnRelease` | state-function | 4 | ok |
| `HGame.HPawn.FollowPatrolPoints` | function | 214 | ok |
| `HGame.HPawn.stateInactive` | state | 1 | ok |
| `HGame.HPawn.stateInfoPrint` | state | 1 | ok |
| `HGame.HPawn.killAttachedParticleFX` | function | 130 | ok |
| `Engine.Pawn.TakeDamage` | function | 698 | ok |
| `Engine.Pawn.Died` | function | 311 | ok |
| `Engine.Pawn.PreBeginPlay` | function | 297 | ok |
| `Engine.Pawn.Destroyed` | function | 200 | ok |
| `Engine.Pawn.PostBeginPlay` | function | 26 | ok |
| `Engine.Pawn.InitPlayerReplicationInfo` | function | 35 | ok |
| `Engine.Pawn.ChangedWeapon` | function | 273 | ok |
| `Engine.Pawn.WarnTarget` | function | 2 | ok |
| `Engine.Pawn.Landed` | function | 73 | ok |
| `Engine.Pawn.JumpOffPawn` | function | 28 | ok |
| `Engine.Pawn.PlayerTimeOut` | function | 20 | ok |
| `Engine.Pawn.LongFall` | function | 2 | ok |
| `Engine.Pawn.PlayTurning` | function | 11 | ok |
| `Engine.Pawn.AdjustHitLocation` | function | 207 | ok |
| `Engine.Pawn.HeadZoneChange` | function | 211 | ok |
| `Engine.Pawn.SpawnGibbedCarcass` | function | 2 | ok |
| `Engine.Pawn.ShakeView` | function | 2 | ok |
| `Engine.Pawn.ClientReStart` | function | 118 | ok |
| `Engine.Pawn.ClientVoiceMessage` | function | 2 | ok |
| `Engine.Pawn.PlayWaiting` | function | 2 | ok |
| `Engine.Pawn.SendVoiceMessage` | function | 287 | ok |
| `Engine.Pawn.PlayDying` | function | 229 | ok |
| `Engine.Pawn.ReceiveLocalizedMessage` | function | 2 | ok |
| `Engine.Pawn.TeamMessage` | function | 2 | ok |
| `Engine.Pawn.ClientMessage` | function | 2 | ok |
| `Engine.Pawn.GameEnded` | state | 1 | ok |
| `Engine.Pawn.GameEnded.BeginState` | state-function | 11 | ok |
| `Engine.Pawn.GameEnded.TakeDamage` | state-function | 2 | ok |
| `Engine.Pawn.GameEnded.Died` | state-function | 2 | ok |
| `Engine.Pawn.GameEnded.WarnTarget` | state-function | 2 | ok |
| `Engine.Pawn.GameEnded.KilledBy` | state-function | 2 | ok |
| `Engine.Pawn.PlayTakeHit` | function | 211 | ok |
| `Engine.Pawn.Dying` | state | 1 | ok |
| `Engine.Pawn.Dying.TakeDamage` | state-function | 87 | ok |
| `Engine.Pawn.Dying.Timer` | state-function | 37 | ok |
| `Engine.Pawn.Dying.BeginState` | state-function | 11 | ok |
| `Engine.Pawn.Dying.LongFall` | state-function | 2 | ok |
| `Engine.Pawn.Dying.Died` | state-function | 2 | ok |
| `Engine.Pawn.Dying.WarnTarget` | state-function | 2 | ok |
| `Engine.Pawn.Dying.Landed` | state-function | 7 | ok |
| `Engine.Pawn.Dying.KilledBy` | state-function | 2 | ok |
| `Engine.Pawn.RenderOverlays` | function | 23 | ok |
| `Engine.Pawn.CheckValidSkinPackage` | function | 6 | ok |
| `Engine.Pawn.PainTimer` | function | 387 | ok |
| `Engine.Pawn.FootZoneChange` | function | 617 | ok |
| `Engine.Pawn.Falling` | function | 6 | ok |
| `Engine.Pawn.damageAttitudeTo` | function | 2 | ok |
| `Engine.Pawn.KillMessage` | function | 59 | ok |
| `Engine.Pawn.UpdateEyeHeight` | function | 2 | ok |
| `Engine.Pawn.Gibbed` | function | 4 | ok |
| `Engine.Pawn.SpawnCarcass` | function | 48 | ok |
| `Engine.Pawn.PlayHit` | function | 2 | ok |
| `Engine.Pawn.NextItem` | function | 103 | ok |
| `Engine.Pawn.AdjustAim` | function | 5 | ok |
| `Engine.Pawn.CutCommand_FollowSpline` | function | 1028 | ok |
| `Engine.Pawn.DoingCutAnimate` | state | 141 | ok |
| `Engine.Pawn.CutCommand_AttachActorToBone` | function | 246 | ok |
| `Engine.Pawn.CutCommand_Animate` | function | 492 | ok |
| `Engine.Pawn.CutCommand_TurnTo` | function | 521 | ok |
| `Engine.Pawn.CutCommand_WalkTo` | function | 574 | ok |
| `Engine.Pawn.DoingTalk` | state | 74 | ok |
| `Engine.Pawn.DoingTalk.AnimEnd` | state-function | 17 | ok |
| `Engine.Pawn.DoingTalk.CutCue` | state-function | 33 | ok |
| `Engine.Pawn.CutCommand_PlayFacialAnim` | function | 292 | ok |
| `Engine.Pawn.CutCommand` | function | 527 | ok |
| `Engine.Pawn.stateSplinePause` | state | 284 | ok |
| `Engine.Pawn.PawnAtInterpolationPoint` | function | 304 | ok |
| `Engine.Pawn.DoCutCueNotify` | function | 36 | ok |
| `Engine.Pawn.stateTurningTo` | state | 53 | ok |
| `Engine.Pawn.RestoreNormalMovementSpeed` | function | 19 | ok |
| `Engine.Pawn.MovingToInit` | function | 174 | ok |
| `Engine.Pawn.stateMovingToLoc` | state | 223 | ok |
| `Engine.Pawn.stateMovingToLoc.EndState` | state-function | 90 | ok |
| `Engine.Pawn.OnEvent` | function | 13 | ok |
| `Engine.Pawn.DoMoveTo` | function | 76 | ok |
| `Engine.Pawn.PlayFacialAnim` | function | 282 | ok |
| `Engine.Pawn.StopHeadLook` | function | 22 | ok |
| `Engine.Pawn.MakeHeadLookInDirectionOfVector` | function | 38 | ok |
| `Engine.Pawn.MakeHeadLookInDirectionOfRotator` | function | 38 | ok |
| `Engine.Pawn.SwitchToBestWeapon` | function | 92 | ok |
| `Engine.Pawn.CheckWaterJump` | function | 222 | ok |
| `Engine.Pawn.SpeechTimer` | function | 2 | ok |
| `Engine.Pawn.WalkTexture` | function | 2 | ok |
| `Engine.Pawn.Killed` | function | 18 | ok |
| `Engine.Pawn.UpdateTactics` | function | 2 | ok |
| `Engine.Pawn.SeePlayer` | function | 2 | ok |
| `Engine.Pawn.HearNoise` | function | 2 | ok |
| `Engine.Pawn.SetMovementPhysics` | function | 2 | ok |
| `Engine.Pawn.CutCommand_UnAttachActor` | function | 256 | ok |
| `Engine.Pawn.AdjustToss` | function | 5 | ok |
| `Engine.Pawn.TakeFallingDamage` | function | 262 | ok |
| `Engine.Pawn.GrabDecoration` | function | 574 | ok |
| `Engine.Pawn.DropDecoration` | function | 77 | ok |
| `Engine.Pawn.PlayTakeHitSound` | function | 125 | ok |
| `Engine.Pawn.TweenToSwimming` | function | 2 | ok |
| `Engine.Pawn.PlayFiring` | function | 2 | ok |
| `Engine.Pawn.PlayLanded` | function | 56 | ok |
| `Engine.Pawn.PlayCrawling` | function | 2 | ok |
| `Engine.Pawn.PlayDuck` | function | 2 | ok |
| `Engine.Pawn.TweenToFalling` | function | 2 | ok |
| `Engine.Pawn.PlayDive` | function | 2 | ok |
| `Engine.Pawn.PlayOutOfWater` | function | 6 | ok |
| `Engine.Pawn.PlayVictoryDance` | function | 11 | ok |
| `Engine.Pawn.CutCommand_Talk` | function | 357 | ok |
| `Engine.Pawn.TraceShot` | function | 102 | ok |
| `Engine.Pawn.PlayRightHit` | function | 9 | ok |
| `Engine.Pawn.PlayGutDeath` | function | 2 | ok |
| `Engine.Pawn.PlayLeftDeath` | function | 2 | ok |
| `Engine.Pawn.PlayHeadDeath` | function | 2 | ok |
| `Engine.Pawn.TweenToWaiting` | function | 9 | ok |
| `Engine.Pawn.TweenToPatrolStop` | function | 9 | ok |
| `Engine.Pawn.TweenToWalking` | function | 9 | ok |
| `Engine.Pawn.TweenToRunning` | function | 11 | ok |
| `Engine.Pawn.PlayWaitingAmbush` | function | 6 | ok |
| `Engine.Pawn.PlayMovingAttack` | function | 6 | ok |
| `Engine.Pawn.SetSkinElement` | function | 410 | ok |
| `Engine.Pawn.GetMultiSkin` | function | 23 | ok |
| `Engine.Pawn.SetMultiSkin` | function | 49 | ok |
| `Engine.Pawn.SetMesh` | function | 9 | ok |
| `Engine.Pawn.EnableEyeBlinks` | function | 53 | ok |
| `Engine.Pawn.PawnCantStandOnMe` | function | 4 | ok |
| `Engine.Pawn.UnderLift` | function | 2 | ok |
| `Engine.Pawn.JumpOffDecoration` | function | 45 | ok |
| `Engine.Pawn.EncroachingOn` | function | 68 | ok |
| `Engine.Pawn.AddInventory` | function | 125 | ok |
| `Engine.Pawn.FindInventoryType` | function | 56 | ok |
| `Engine.Pawn.TossWeapon` | function | 73 | ok |
| `Engine.Pawn.AdjustDesireFor` | function | 8 | ok |
| `Engine.Pawn.ClientGameEnded` | function | 7 | ok |
| `Engine.Pawn.ClientDying` | function | 17 | ok |
| `Engine.Pawn.ClientSetRotation` | function | 28 | ok |
| `Engine.Pawn.ClientSetLocation` | function | 106 | ok |
| `Engine.Pawn.AddVelocity` | function | 67 | ok |
| `Engine.Pawn.GetRating` | function | 8 | ok |
| `Engine.Pawn.FearThisSpot` | function | 2 | ok |
| `Engine.Pawn.HandleHelpMessageFrom` | function | 2 | ok |
| `Engine.Pawn.BotVoiceMessage` | function | 2 | ok |
| `Engine.Pawn.SendTeamMessage` | function | 20 | ok |
| `Engine.Pawn.TeamBroadcast` | function | 362 | ok |
| `Engine.Pawn.CheckFutureSight` | function | 4 | ok |
| `Engine.Pawn.patrolFollowSpline` | state | 42 | ok |
| `Engine.Pawn.patrolFollowSpline.EndState` | state-function | 26 | ok |
| `Engine.Pawn.patrolFollowSpline.ShouldPlayIdleOnRelease` | state-function | 4 | ok |
| `Engine.Pawn.PlayRecoil` | function | 2 | ok |
| `Engine.Pawn.FellOutOfWorld` | function | 42 | ok |
| `Engine.Pawn.ClientPutDown` | function | 16 | ok |
| `Engine.Pawn.Mount` | function | 2 | ok |
| `Engine.Pawn.AlterDestination` | function | 2 | ok |
| `Engine.Pawn.MayFall` | function | 2 | ok |
| `Engine.Pawn.PickAnyTarget` | function | 12 | ok |
| `Engine.Pawn.SplineTick` | function | 339 | ok |
| `Engine.Pawn.FindStairRotation` | function | 3 | ok |
| `Engine.Pawn.pointReachable` | function | 3 | ok |
| `Engine.Pawn.CalcSplineSpeedFromTime` | function | 309 | ok |
| `Engine.Pawn.DestroyControllers` | function | 42 | ok |
| `Engine.Pawn.FollowSplinePath` | function | 767 | ok |
| `Engine.Pawn.PlayIdleAnim` | function | 14 | ok |
| `Engine.Pawn.FindRandomDest` | function | 3 | ok |
| `Engine.Pawn.FindPath` | function | 6 | ok |
| `Engine.Pawn.DestroyTurnToPermanentController` | function | 41 | ok |
| `Engine.Pawn.FindPathToward` | function | 9 | ok |
| `Engine.Pawn.FindPathTo` | function | 9 | ok |
| `Engine.Pawn.CanSee` | function | 3 | ok |
| `Engine.Pawn.LineOfSightTo` | function | 3 | ok |
| `Engine.Pawn.MakeTurnToPermanentController` | function | 24 | ok |
| `Engine.Pawn.DoTurnTo` | function | 94 | ok |
| `Engine.Pawn.TurnTo` | function | 3 | ok |
| `Engine.Pawn.DoWalkTo` | function | 34 | ok |
| `Engine.Pawn.DoRunTo` | function | 34 | ok |
| `Engine.Pawn.StrafeFacing` | function | 6 | ok |
| `Engine.Pawn.StrafeTo` | function | 6 | ok |
| `Engine.Pawn.MoveToward` | function | 6 | ok |
| `Engine.Pawn.MoveTo` | function | 6 | ok |
| `Engine.Pawn.MakeHeadWatchActor` | function | 41 | ok |
| `Engine.Pawn.MakeHeadWatchPosition` | function | 38 | ok |
| `Engine.Pawn.MakeHeadLookAtActor` | function | 41 | ok |
| `Engine.Pawn.MakeHeadLookAtPosition` | function | 38 | ok |
| `Engine.Pawn.EnemyNotVisible` | function | 2 | ok |
| `Engine.Pawn.HidePlayer` | function | 23 | ok |
| `Engine.Pawn.PlayDeathHit` | function | 2 | ok |
| `Engine.Pawn.StopFiring` | function | 2 | ok |
| `Engine.Pawn.Gasp` | function | 2 | ok |
| `Engine.Pawn.PlayWeaponSwitch` | function | 2 | ok |
| `Engine.Pawn.PlayInAir` | function | 2 | ok |
| `Engine.Pawn.FireWeapon` | function | 2 | ok |
| `Engine.Pawn.PlayLeftHit` | function | 9 | ok |
| `Engine.Pawn.PlayHeadHit` | function | 9 | ok |
| `Engine.Pawn.PlayGutHit` | function | 66 | ok |
| `Engine.Pawn.PlayRightDeath` | function | 2 | ok |
| `Engine.Pawn.PlayBigDeath` | function | 2 | ok |
| `Engine.Pawn.PlayPatrolStop` | function | 6 | ok |
| `Engine.Pawn.PlayThreatening` | function | 11 | ok |
| `Engine.Pawn.TweenToFighter` | function | 2 | ok |
| `Engine.Pawn.PlayWalking` | function | 6 | ok |
| `Engine.Pawn.PlayRunning` | function | 2 | ok |
| `Engine.Pawn.PreSetMovement` | function | 81 | ok |
| `Engine.Pawn.CreateEyeBlinkAnimChannel` | function | 384 | ok |
| `Engine.Pawn.CreateHeadLookAnimChannel` | function | 250 | ok |
| `Engine.Pawn.PostNetBeginPlay` | function | 124 | ok |
| `Engine.Pawn.ClientHearSound` | function | 21 | ok |
| `Engine.Pawn.BaseChange` | function | 280 | ok |
| `Engine.Pawn.gibbedBy` | function | 66 | ok |
| `Engine.Pawn.EncroachedBy` | function | 20 | ok |
| `Engine.Pawn.DeleteInventory` | function | 112 | ok |
| `Engine.Pawn.SendGlobalMessage` | function | 20 | ok |
| `Engine.Pawn.RestartPlayer` | function | 2 | ok |
| `Engine.Pawn.SpecialFire` | function | 2 | ok |
| `Engine.Pawn.BecomeViewTarget` | function | 8 | ok |
| `Engine.Pawn.SetDefaultDisplayProperties` | function | 131 | ok |
| `Engine.Pawn.SetDisplayProperties` | function | 107 | ok |
| `Engine.Pawn.GetHumanName` | function | 28 | ok |
| `Engine.Pawn.PickTarget` | function | 12 | ok |
| `Engine.Pawn.FindBestInventoryPath` | function | 6 | ok |
| `Engine.Pawn.actorReachable` | function | 3 | ok |
| `Engine.Pawn.TurnToward` | function | 3 | ok |
| `HGame.Characters.Tick` | function | 340 | ok |
| `HGame.Characters.PostBeginPlay` | function | 339 | ok |
| `HGame.Characters.Bump` | function | 148 | ok |
| `HGame.Characters.patrol` | state | 1 | ok |
| `HGame.Characters.patrol.startup` | state-function | 26 | ok |
| `HGame.Characters.VendorInit` | function | 2199 | ok |
| `HGame.Characters.GetVendorHarryInquiryId` | function | 8 | ok |
| `HGame.Characters.GetVendorLureId` | function | 8 | ok |
| `HGame.Characters.GetVendorNotEnoughBeansId` | function | 8 | ok |
| `HGame.Characters.GetVendorDeclineId` | function | 8 | ok |
| `HGame.Characters.GetSellingPrice` | function | 93 | ok |
| `HGame.Characters.HaveSomethingToSell` | function | 147 | ok |
| `HGame.Characters.VendorGive` | state | 59 | ok |
| `HGame.Characters.VendorEngaged` | state | 1 | ok |
| `HGame.Characters.GetTalkAnimName` | function | 36 | ok |
| `HGame.Characters.SayOutOfStockLine` | state | 238 | ok |
| `HGame.Characters.SayOutOfStockLine.CutCue` | state-function | 39 | ok |
| `HGame.Characters.SayOutOfStockLine.BeginState` | state-function | 6 | ok |
| `HGame.Characters.SayOutOfStockLine.EndState` | state-function | 2 | ok |
| `HGame.Characters.VendorIdle` | state | 44 | ok |
| `HGame.Characters.VendorIdle.BeginState` | state-function | 151 | ok |
| `HGame.Characters.VendorIdle.EndState` | state-function | 12 | ok |
| `HGame.Characters.SetEverythingForTheDuel` | function | 557 | ok |
| `HGame.Characters.OnResolveGameState` | function | 30 | ok |
| `HGame.Characters.GetVendorInstructionId` | function | 8 | ok |
| `HGame.Characters.GetVendorRanOutOfBeansId` | function | 8 | ok |
| `HGame.Characters.GetVendorTransactionDoneId` | function | 8 | ok |
| `HGame.Characters.GetVendorOutOfStockId` | function | 8 | ok |
| `HGame.Characters.GetSellDialogId` | function | 136 | ok |
| `HGame.Characters.CutCommand` | function | 70 | ok |
| `HGame.Characters.InterruptOtherVendorPopup` | function | 35 | ok |
| `HGame.Characters.IsInVendorPopupState` | function | 20 | ok |
| `HGame.Characters.OtherVendorInterruptedPopup` | function | 27 | ok |
| `HGame.Characters.SellsSomethingButOutOfStock` | function | 23 | ok |
| `HGame.Characters.IsWizardCardAvailable` | function | 13 | ok |
| `HGame.Characters.GetAvailableWizardCardId` | function | 75 | ok |
| `HGame.Characters.OnHarryCaptured` | function | 27 | ok |
| `HGame.Characters.GetWeasleyTwin` | function | 119 | ok |
| `HGame.Characters.MakePurchase` | function | 655 | ok |
| `HGame.Characters.SayPopupLine` | function | 224 | ok |
| `HGame.Characters.SayVendorLureLine` | state | 234 | ok |
| `HGame.Characters.SayVendorLureLine.BeginState` | state-function | 6 | ok |
| `HGame.Characters.SayVendorLureLine.CutCue` | state-function | 35 | ok |
| `HGame.Characters.SayVendorLureLine.EndState` | state-function | 2 | ok |
| `HGame.Characters.IsDuelVendor` | function | 41 | ok |
| `HGame.HChar.Tick` | function | 96 | ok |
| `HGame.HChar.PreBeginPlay` | function | 385 | ok |
| `HGame.HChar.stateIdle` | state | 125 | ok |
| `HGame.HChar.HandleSpellFlipendo` | function | 59 | ok |
| `HGame.HChar.Landed` | function | 73 | ok |
| `HGame.HChar.Bump` | function | 230 | ok |
| `HGame.HChar.CutCommand` | function | 60 | ok |
| `HGame.HChar.OnEvent` | function | 13 | ok |
| `HGame.HChar.IsHuntingHarry` | function | 4 | ok |
| `HGame.HChar.ShouldStartLookingForHarry` | function | 4 | ok |
| `HGame.HChar.GetHealth` | function | 7 | ok |
| `HGame.HChar.patrol` | state | 1 | ok |
| `HGame.HChar.CanSeeHarry` | function | 111 | ok |
| `HGame.HChar.StartFollowingHarry` | state | 36 | ok |
| `HGame.HChar.StartFollowingHarry.ShouldStartLookingForHarry` | state-function | 4 | ok |
| `HGame.HChar.StartFollowingHarry.IsHuntingHarry` | state-function | 4 | ok |
| `HGame.HChar.StartFollowingHarry.Tick` | state-function | 30 | ok |
| `HGame.HChar.DoAction` | function | 2 | ok |
| `HGame.HChar.SaveState` | function | 9 | ok |
| `HGame.HChar.PlayRandomSoundAndAnimFirstTime` | function | 93 | ok |
| `HGame.HChar.PlayRandomSoundAndAnimSecondTime` | function | 97 | ok |
| `HGame.HChar.followHarry` | state | 109 | ok |
| `HGame.HChar.followHarry.ShouldStartLookingForHarry` | state-function | 4 | ok |
| `HGame.HChar.followHarry.IsHuntingHarry` | state-function | 4 | ok |
| `HGame.HChar.RandomLookForHarry` | state | 128 | ok |
| `HGame.HChar.RandomLookForHarry.ShouldStartLookingForHarry` | state-function | 4 | ok |
| `HGame.HChar.RandomLookForHarry.IsHuntingHarry` | state-function | 4 | ok |
| `HGame.HChar.CaughtHarry` | state | 1 | ok |
| `HGame.HChar.CaughtHarry.ShouldStartLookingForHarry` | state-function | 4 | ok |
| `HGame.HChar.CaughtHarry.IsHuntingHarry` | state-function | 4 | ok |
| `HGame.HChar.SaySomethingFirstTime` | state | 198 | ok |
| `HGame.HChar.SaySomethingFirstTime.ShouldStartLookingForHarry` | state-function | 4 | ok |
| `HGame.HChar.SaySomethingFirstTime.IsHuntingHarry` | state-function | 4 | ok |
| `HGame.HChar.SaySomethingFirstTime.Tick` | state-function | 47 | ok |
| `HGame.HChar.SaySomethingFirstTime.BeginState` | state-function | 34 | ok |
| `HGame.HChar.NotifyOthersOfHarry` | function | 88 | ok |
| `HGame.HChar.GetCurrFidgetAnimName` | function | 61 | ok |
| `HGame.HChar.GetCurrIdleAnimName` | function | 81 | ok |
| `HGame.HChar.DoBumpLine` | function | 1070 | ok |
| `HGame.HChar.DoingBumpLine` | state | 31 | ok |
| `HGame.HChar.DoingBumpLine.BeginState` | state-function | 49 | ok |
| `HGame.HChar.DoingBumpLine.Bump` | state-function | 9 | ok |
| `HGame.HChar.DoingBumpLine.CutCue` | state-function | 80 | ok |
| `HGame.HChar.ObjectPickup` | function | 135 | ok |
| `HGame.HChar.ObjectThrow` | function | 159 | ok |
| `HGame.HChar.DoPickup` | function | 2 | ok |
| `HGame.HChar.DoAttack` | function | 2 | ok |
| `HGame.HChar.DoPossess` | function | 2 | ok |
| `HGame.HChar.DoUnPossess` | function | 2 | ok |
| `HGame.HChar.CanAttack` | function | 2 | ok |
| `HGame.HChar.OnTouch` | function | 2 | ok |
| `HGame.HChar.CutCommand_HandleSet` | function | 190 | ok |
| `HGame.HChar.PlayFootStep` | function | 443 | ok |
| `HGame.HChar.RestoreState` | function | 7 | ok |
| `HGame.HChar.GetRandomFallSound` | function | 65 | ok |
| `HGame.HChar.HandleFallSounds` | function | 114 | ok |
| `Engine.SpecialEvent.Trigger` | function | 58 | ok |
| `Engine.SpecialEvent.DisplayMessage` | state | 1 | ok |
| `Engine.SpecialEvent.DamageInstigator` | state | 1 | ok |
| `Engine.SpecialEvent.DamageInstigator.Trigger` | state-function | 84 | ok |
| `Engine.SpecialEvent.KillInstigator` | state | 1 | ok |
| `Engine.SpecialEvent.KillInstigator.Trigger` | state-function | 76 | ok |
| `Engine.SpecialEvent.PlaySoundEffect` | state | 1 | ok |
| `Engine.SpecialEvent.PlaySoundEffect.Trigger` | state-function | 64 | ok |
| `Engine.SpecialEvent.PlayersPlaySoundEffect` | state | 1 | ok |
| `Engine.SpecialEvent.PlayersPlaySoundEffect.Trigger` | state-function | 94 | ok |
| `Engine.SpecialEvent.PlayAmbientSoundEffect` | state | 1 | ok |
| `Engine.SpecialEvent.PlayAmbientSoundEffect.Trigger` | state-function | 72 | ok |
| `Engine.SpecialEvent.PlayerPath` | state | 1 | ok |
| `Engine.SpecialEvent.PlayerPath.Trigger` | state-function | 205 | ok |
| `Engine.Triggers.OnResolveGameState` | function | 27 | ok |
| `Engine.ParticleFX.SetParticleParams` | function | 6 | ok |
| `Engine.ParticleFX.RecomputeDeltas` | function | 3 | ok |
| `Engine.ParticleFX.CutCommand` | function | 139 | ok |
| `Engine.ParticleFX.GetParticleParams` | function | 6 | ok |
| `Engine.ParticleFX.AddParticle` | function | 6 | ok |
| `Engine.ParticleFX.Shutdown` | function | 24 | ok |
| `Engine.ParticleFX.EnableEmission` | function | 47 | ok |
| `Engine.PlayerPawn.PlayerTick` | function | 2 | ok |
| `Engine.PlayerPawn.Fire` | function | 161 | ok |
| `Engine.PlayerPawn.ProcessMove` | function | 8 | ok |
| `Engine.PlayerPawn.AltFire` | function | 161 | ok |
| `Engine.PlayerPawn.GetDefaultURL` | function | 3 | ok |
| `Engine.PlayerPawn.Suicide` | function | 7 | ok |
| `Engine.PlayerPawn.Taunt` | function | 39 | ok |
| `Engine.PlayerPawn.KilledBy` | function | 18 | ok |
| `Engine.PlayerPawn.Died` | function | 19 | ok |
| `Engine.PlayerPawn.Landed` | function | 51 | ok |
| `Engine.PlayerPawn.ChangeTeam` | function | 84 | ok |
| `Engine.PlayerPawn.SwitchWeapon` | function | 208 | ok |
| `Engine.PlayerPawn.MoveAutonomous` | function | 80 | ok |
| `Engine.PlayerPawn.ServerMove` | function | 1271 | ok |
| `Engine.PlayerPawn.InitPlayerReplicationInfo` | function | 20 | ok |
| `Engine.PlayerPawn.ThrowWeapon` | function | 149 | ok |
| `Engine.PlayerPawn.PlayChatting` | function | 2 | ok |
| `Engine.PlayerPawn.ReceiveLocalizedMessage` | function | 26 | ok |
| `Engine.PlayerPawn.ClientMessage` | function | 118 | ok |
| `Engine.PlayerPawn.TeamMessage` | function | 92 | ok |
| `Engine.PlayerPawn.ClientVoiceMessage` | function | 99 | ok |
| `Engine.PlayerPawn.GetFreeMove` | function | 57 | ok |
| `Engine.PlayerPawn.ServerReStartGame` | function | 2 | ok |
| `Engine.PlayerPawn.ServerFeignDeath` | function | 2 | ok |
| `Engine.PlayerPawn.ServerReStartPlayer` | function | 2 | ok |
| `Engine.PlayerPawn.ServerChangeSkin` | function | 52 | ok |
| `Engine.PlayerPawn.Jump` | function | 49 | ok |
| `Engine.PlayerPawn.FeignDeath` | function | 2 | ok |
| `Engine.PlayerPawn.CallForHelp` | function | 151 | ok |
| `Engine.PlayerPawn.Grab` | function | 21 | ok |
| `Engine.PlayerPawn.Say` | function | 361 | ok |
| `Engine.PlayerPawn.RestartLevel` | function | 44 | ok |
| `Engine.PlayerPawn.PrevItem` | function | 247 | ok |
| `Engine.PlayerPawn.ReplaceText` | function | 108 | ok |
| `Engine.PlayerPawn.ViewPlayer` | function | 201 | ok |
| `Engine.PlayerPawn.ViewClass` | function | 365 | ok |
| `Engine.PlayerPawn.Fly` | function | 92 | ok |
| `Engine.PlayerPawn.Walk` | function | 35 | ok |
| `Engine.PlayerPawn.BehindView` | function | 11 | ok |
| `Engine.PlayerPawn.UpdateEyeHeight` | function | 448 | ok |
| `Engine.PlayerPawn.PlayerTimeOut` | function | 20 | ok |
| `Engine.PlayerPawn.ChangedWeapon` | function | 27 | ok |
| `Engine.PlayerPawn.Possess` | function | 79 | ok |
| `Engine.PlayerPawn.PostBeginPlay` | function | 155 | ok |
| `Engine.PlayerPawn.PlayerCalcView` | function | 282 | ok |
| `Engine.PlayerPawn.AdminLogin` | function | 22 | ok |
| `Engine.PlayerPawn.PlayBeepSound` | function | 2 | ok |
| `Engine.PlayerPawn.Thumbnail` | function | 264 | ok |
| `Engine.PlayerPawn.CheckBob` | function | 275 | ok |
| `Engine.PlayerPawn.AdminLogout` | function | 19 | ok |
| `Engine.PlayerPawn.RenderOverlays` | function | 44 | ok |
| `Engine.PlayerPawn.SShot` | function | 218 | ok |
| `Engine.PlayerPawn.Admin` | function | 34 | ok |
| `Engine.PlayerPawn.PlayerList` | function | 70 | ok |
| `Engine.PlayerPawn.Profile` | function | 238 | ok |
| `Engine.PlayerPawn.ViewPlayerNum` | function | 415 | ok |
| `Engine.PlayerPawn.ClientReplicateSkins` | function | 58 | ok |
| `Engine.PlayerPawn.CompressAccel` | function | 48 | ok |
| `Engine.PlayerPawn.ReplicateMove` | function | 1834 | ok |
| `Engine.PlayerPawn.Destroyed` | function | 114 | ok |
| `Engine.PlayerPawn.ClientAdjustPosition` | function | 225 | ok |
| `Engine.PlayerPawn.ClientFadeIn` | function | 62 | ok |
| `Engine.PlayerPawn.ClientUpdatePosition` | function | 494 | ok |
| `Engine.PlayerPawn.damageAttitudeTo` | function | 18 | ok |
| `Engine.PlayerPawn.ClientWeaponEvent` | function | 23 | ok |
| `Engine.PlayerPawn.Typing` | function | 201 | ok |
| `Engine.PlayerPawn.Speech` | function | 48 | ok |
| `Engine.PlayerPawn.TeamSay` | function | 297 | ok |
| `Engine.PlayerPawn.StartZoom` | function | 17 | ok |
| `Engine.PlayerPawn.LocalTravel` | function | 37 | ok |
| `Engine.PlayerPawn.StopZoom` | function | 8 | ok |
| `Engine.PlayerPawn.NextWeapon` | function | 645 | ok |
| `Engine.PlayerPawn.SetPause` | function | 24 | ok |
| `Engine.PlayerPawn.KickBan` | function | 329 | ok |
| `Engine.PlayerPawn.ActivateHint` | function | 48 | ok |
| `Engine.PlayerPawn.ActivateTranslator` | function | 48 | ok |
| `Engine.PlayerPawn.ActivateItem` | function | 49 | ok |
| `Engine.PlayerPawn.HandleWalking` | function | 199 | ok |
| `Engine.PlayerPawn.PlayHit` | function | 20 | ok |
| `Engine.PlayerPawn.ConsoleCommand` | function | 3 | ok |
| `Engine.PlayerPawn.AlwaysMouseLook` | function | 13 | ok |
| `Engine.PlayerPawn.SetFOVAngle` | function | 9 | ok |
| `Engine.PlayerPawn.ClientFlash` | function | 23 | ok |
| `Engine.PlayerPawn.ClientInstantFlash` | function | 23 | ok |
| `Engine.PlayerPawn.ClientPlaySound` | function | 132 | ok |
| `Engine.PlayerPawn.ChangeSnapView` | function | 11 | ok |
| `Engine.PlayerPawn.SnapView` | function | 13 | ok |
| `Engine.PlayerPawn.StairLook` | function | 13 | ok |
| `Engine.PlayerPawn.ClientFadeOut` | function | 62 | ok |
| `Engine.PlayerPawn.ClientShake` | function | 2 | ok |
| `Engine.PlayerPawn.ClientReliablePlaySound` | function | 17 | ok |
| `Engine.PlayerPawn.ClientAdjustGlow` | function | 25 | ok |
| `Engine.PlayerPawn.ShakeView` | function | 47 | ok |
| `Engine.PlayerPawn.ShowSpecialMenu` | function | 43 | ok |
| `Engine.PlayerPawn.ClientSetMusic` | function | 37 | ok |
| `Engine.PlayerPawn.ServerSetHandedness` | function | 30 | ok |
| `Engine.PlayerPawn.ServerTaunt` | function | 18 | ok |
| `Engine.PlayerPawn.CauseEvent` | function | 78 | ok |
| `Engine.PlayerPawn.Ping` | function | 34 | ok |
| `Engine.PlayerPawn.PreClientTravel` | function | 2 | ok |
| `Engine.PlayerPawn.ToggleZoom` | function | 24 | ok |
| `Engine.PlayerPawn.PrevWeapon` | function | 644 | ok |
| `Engine.PlayerPawn.EndZoom` | function | 15 | ok |
| `Engine.PlayerPawn.FOV` | function | 9 | ok |
| `Engine.PlayerPawn.SetDesiredFOV` | function | 86 | ok |
| `Engine.PlayerPawn.SaveGame` | function | 117 | ok |
| `Engine.PlayerPawn.Mutate` | function | 48 | ok |
| `Engine.PlayerPawn.QuickSave` | function | 64 | ok |
| `Engine.PlayerPawn.Kick` | function | 136 | ok |
| `Engine.PlayerPawn.QuickLoad` | function | 58 | ok |
| `Engine.PlayerPawn.SetMouseSmoothThreshold` | function | 24 | ok |
| `Engine.PlayerPawn.SetMaxMouseSmoothing` | function | 14 | ok |
| `Engine.PlayerPawn.ActivateInventoryItem` | function | 33 | ok |
| `Engine.PlayerPawn.Pause` | function | 39 | ok |
| `Engine.PlayerPawn.PostRender` | function | 58 | ok |
| `Engine.PlayerPawn.ChangeHud` | function | 30 | ok |
| `Engine.PlayerPawn.PreRender` | function | 58 | ok |
| `Engine.PlayerPawn.ChangeCrosshair` | function | 30 | ok |
| `Engine.PlayerPawn.FunctionKey` | function | 2 | ok |
| `Engine.PlayerPawn.GetWeapon` | function | 192 | ok |
| `Engine.PlayerPawn.DoJump` | function | 229 | ok |
| `Engine.PlayerPawn.SetNGSecret` | function | 9 | ok |
| `Engine.PlayerPawn.InLumosRadius` | function | 17 | ok |
| `Engine.PlayerPawn.CopyToClipboard` | function | 3 | ok |
| `Engine.PlayerPawn.InvertMouse` | function | 14 | ok |
| `Engine.PlayerPawn.ServerNeverSwitchOnPickup` | function | 11 | ok |
| `Engine.PlayerPawn.SwitchLevel` | function | 40 | ok |
| `Engine.PlayerPawn.AutoJump` | function | 14 | ok |
| `Engine.PlayerPawn.ShowScores` | function | 13 | ok |
| `Engine.PlayerPawn.UpdateURL` | function | 9 | ok |
| `Engine.PlayerPawn.ChangeAlwaysMouseLook` | function | 24 | ok |
| `Engine.PlayerPawn.Amphibious` | function | 51 | ok |
| `Engine.PlayerPawn.ShowUpgradeMenu` | function | 2 | ok |
| `Engine.PlayerPawn.SetWeaponStay` | function | 89 | ok |
| `Engine.PlayerPawn.ClientTravel` | function | 9 | ok |
| `Engine.PlayerPawn.PlayerWaking` | state | 1 | ok |
| `Engine.PlayerPawn.PlayerWaking.PlayerTick` | state-function | 19 | ok |
| `Engine.PlayerPawn.PlayerWaking.BeginState` | state-function | 57 | ok |
| `Engine.PlayerPawn.PlayerWaking.PlayerMove` | state-function | 134 | ok |
| `Engine.PlayerPawn.PlayerWaking.Timer` | state-function | 7 | ok |
| `Engine.PlayerPawn.PlayerWaking.SwitchWeapon` | state-function | 2 | ok |
| `Engine.PlayerPawn.PlayerWaking.KilledBy` | state-function | 2 | ok |
| `Engine.PlayerPawn.Ghost` | function | 90 | ok |
| `Engine.PlayerPawn.AllAmmo` | function | 119 | ok |
| `Engine.PlayerPawn.SetBob` | function | 12 | ok |
| `Engine.PlayerPawn.ChangeStairLook` | function | 24 | ok |
| `Engine.PlayerPawn.SetDodgeClickTime` | function | 12 | ok |
| `Engine.PlayerPawn.Name` | function | 9 | ok |
| `Engine.PlayerPawn.ChangeDodgeClickTime` | function | 16 | ok |
| `Engine.PlayerPawn.SetName` | function | 61 | ok |
| `Engine.PlayerPawn.ChangeName` | function | 23 | ok |
| `Engine.PlayerPawn.ClientChangeTeam` | function | 144 | ok |
| `Engine.PlayerPawn.PlayerWaiting` | state | 1 | ok |
| `Engine.PlayerPawn.PlayerWaiting.BeginState` | state-function | 55 | ok |
| `Engine.PlayerPawn.PlayerWaiting.PlayWaiting` | state-function | 2 | ok |
| `Engine.PlayerPawn.PlayerWaiting.PlayerTick` | state-function | 19 | ok |
| `Engine.PlayerPawn.PlayerWaiting.AltFire` | state-function | 8 | ok |
| `Engine.PlayerPawn.PlayerWaiting.ChangeTeam` | state-function | 22 | ok |
| `Engine.PlayerPawn.PlayerWaiting.Jump` | state-function | 2 | ok |
| `Engine.PlayerPawn.PlayerWaiting.Died` | state-function | 2 | ok |
| `Engine.PlayerPawn.PlayerWaiting.TakeDamage` | state-function | 2 | ok |
| `Engine.PlayerPawn.PlayerWaiting.EndState` | state-function | 36 | ok |
| `Engine.PlayerPawn.PlayerWaiting.PlayerMove` | state-function | 179 | ok |
| `Engine.PlayerPawn.PlayerWaiting.ProcessMove` | state-function | 18 | ok |
| `Engine.PlayerPawn.PlayerWaiting.Fire` | state-function | 8 | ok |
| `Engine.PlayerPawn.PlayerWaiting.Suicide` | state-function | 2 | ok |
| `Engine.PlayerPawn.SetAutoAim` | function | 12 | ok |
| `Engine.PlayerPawn.ChangeAutoAim` | function | 26 | ok |
| `Engine.PlayerPawn.PlayersOnly` | function | 44 | ok |
| `Engine.PlayerPawn.SetSensitivity` | function | 12 | ok |
| `Engine.PlayerPawn.SetHand` | function | 12 | ok |
| `Engine.PlayerPawn.SetGameState` | function | 746 | ok |
| `Engine.PlayerPawn.ChangeSetHand` | function | 115 | ok |
| `Engine.PlayerPawn.GetGameStateMasterListToken` | function | 36 | ok |
| `Engine.PlayerPawn.CheatView` | function | 299 | ok |
| `Engine.PlayerPawn.GetNGSecret` | function | 6 | ok |
| `Engine.PlayerPawn.ViewSelf` | function | 27 | ok |
| `Engine.PlayerPawn.NeverSwitchOnPickup` | function | 31 | ok |
| `Engine.PlayerPawn.SetJumpZ` | function | 49 | ok |
| `Engine.PlayerPawn.PlayerFlying` | state | 1 | ok |
| `Engine.PlayerPawn.PlayerFlying.AnimEnd` | state-function | 6 | ok |
| `Engine.PlayerPawn.PlayerFlying.PlayerTick` | state-function | 19 | ok |
| `Engine.PlayerPawn.PlayerFlying.BeginState` | state-function | 19 | ok |
| `Engine.PlayerPawn.PlayerFlying.PlayerMove` | state-function | 149 | ok |
| `Engine.PlayerPawn.SwitchCoopLevel` | function | 40 | ok |
| `Engine.PlayerPawn.KillPawns` | function | 66 | ok |
| `Engine.PlayerPawn.KillAll` | function | 72 | ok |
| `Engine.PlayerPawn.SetSpeed` | function | 66 | ok |
| `Engine.PlayerPawn.RememberSpot` | function | 8 | ok |
| `Engine.PlayerPawn.Summon` | function | 124 | ok |
| `Engine.PlayerPawn.TravelPostAccept` | function | 15 | ok |
| `Engine.PlayerPawn.GameEnded` | state | 1 | ok |
| `Engine.PlayerPawn.GameEnded.BeginState` | state-function | 137 | ok |
| `Engine.PlayerPawn.GameEnded.ServerReStartGame` | state-function | 18 | ok |
| `Engine.PlayerPawn.GameEnded.Taunt` | state-function | 17 | ok |
| `Engine.PlayerPawn.GameEnded.Timer` | state-function | 8 | ok |
| `Engine.PlayerPawn.GameEnded.FindGoodView` | state-function | 173 | ok |
| `Engine.PlayerPawn.GameEnded.ServerMove` | state-function | 88 | ok |
| `Engine.PlayerPawn.GameEnded.ViewPlayer` | state-function | 2 | ok |
| `Engine.PlayerPawn.GameEnded.PlayerMove` | state-function | 285 | ok |
| `Engine.PlayerPawn.GameEnded.AltFire` | state-function | 8 | ok |
| `Engine.PlayerPawn.GameEnded.PlayerTick` | state-function | 19 | ok |
| `Engine.PlayerPawn.GameEnded.Fire` | state-function | 50 | ok |
| `Engine.PlayerPawn.GameEnded.ViewClass` | state-function | 2 | ok |
| `Engine.PlayerPawn.GameEnded.ThrowWeapon` | state-function | 2 | ok |
| `Engine.PlayerPawn.GameEnded.Suicide` | state-function | 2 | ok |
| `Engine.PlayerPawn.GameEnded.Died` | state-function | 2 | ok |
| `Engine.PlayerPawn.GameEnded.TakeDamage` | state-function | 2 | ok |
| `Engine.PlayerPawn.GameEnded.KilledBy` | state-function | 2 | ok |
| `Engine.PlayerPawn.ShowMenu` | function | 66 | ok |
| `Engine.PlayerPawn.ShowLoadMenu` | function | 6 | ok |
| `Engine.PlayerPawn.AddBots` | function | 9 | ok |
| `Engine.PlayerPawn.ServerAddBots` | function | 94 | ok |
| `Engine.PlayerPawn.ClearProgressMessages` | function | 74 | ok |
| `Engine.PlayerPawn.SetProgressMessage` | function | 23 | ok |
| `Engine.PlayerPawn.SetProgressColor` | function | 23 | ok |
| `Engine.PlayerPawn.SetProgressTime` | function | 19 | ok |
| `Engine.PlayerPawn.UnPossess` | function | 62 | ok |
| `Engine.PlayerPawn.ServerSetWeaponPriority` | function | 82 | ok |
| `Engine.PlayerPawn.SpawnCarcass` | function | 101 | ok |
| `Engine.PlayerPawn.CalcBehindView` | function | 118 | ok |
| `Engine.PlayerPawn.StartWalk` | function | 30 | ok |
| `Engine.PlayerPawn.Dying` | state | 1 | ok |
| `Engine.PlayerPawn.Dying.EndState` | state-function | 136 | ok |
| `Engine.PlayerPawn.Dying.AltFire` | state-function | 8 | ok |
| `Engine.PlayerPawn.Dying.PlayChatting` | state-function | 2 | ok |
| `Engine.PlayerPawn.Dying.Taunt` | state-function | 2 | ok |
| `Engine.PlayerPawn.Dying.ServerReStartPlayer` | state-function | 149 | ok |
| `Engine.PlayerPawn.Dying.Suicide` | state-function | 2 | ok |
| `Engine.PlayerPawn.Dying.Timer` | state-function | 20 | ok |
| `Engine.PlayerPawn.Dying.BeginState` | state-function | 149 | ok |
| `Engine.PlayerPawn.Dying.TakeDamage` | state-function | 30 | ok |
| `Engine.PlayerPawn.Dying.FindGoodView` | state-function | 150 | ok |
| `Engine.PlayerPawn.Dying.PlayerTick` | state-function | 19 | ok |
| `Engine.PlayerPawn.Dying.PlayerMove` | state-function | 241 | ok |
| `Engine.PlayerPawn.Dying.PlayerCalcView` | state-function | 350 | ok |
| `Engine.PlayerPawn.Dying.ServerMove` | state-function | 31 | ok |
| `Engine.PlayerPawn.Dying.Fire` | state-function | 86 | ok |
| `Engine.PlayerPawn.Dying.SwitchWeapon` | state-function | 2 | ok |
| `Engine.PlayerPawn.Dying.KilledBy` | state-function | 2 | ok |
| `Engine.PlayerPawn.ShowInventory` | function | 161 | ok |
| `Engine.PlayerPawn.PlayerSpectating` | state | 1 | ok |
| `Engine.PlayerPawn.PlayerSpectating.BeginState` | state-function | 53 | ok |
| `Engine.PlayerPawn.PlayerSpectating.PlayerTick` | state-function | 19 | ok |
| `Engine.PlayerPawn.PlayerSpectating.Fire` | state-function | 28 | ok |
| `Engine.PlayerPawn.PlayerSpectating.ProcessMove` | state-function | 18 | ok |
| `Engine.PlayerPawn.PlayerSpectating.Suicide` | state-function | 2 | ok |
| `Engine.PlayerPawn.PlayerSpectating.Died` | state-function | 2 | ok |
| `Engine.PlayerPawn.PlayerSpectating.EndState` | state-function | 36 | ok |
| `Engine.PlayerPawn.PlayerSpectating.PlayerMove` | state-function | 179 | ok |
| `Engine.PlayerPawn.PlayerSpectating.ChangeTeam` | state-function | 22 | ok |
| `Engine.PlayerPawn.PlayerSpectating.AltFire` | state-function | 27 | ok |
| `Engine.PlayerPawn.PlayerSpectating.SendVoiceMessage` | state-function | 2 | ok |
| `Engine.PlayerPawn.PlayerSpectating.TakeDamage` | state-function | 2 | ok |
| `Engine.PlayerPawn.Invisible` | function | 77 | ok |
| `Engine.PlayerPawn.God` | function | 101 | ok |
| `Engine.PlayerPawn.UpdateBob` | function | 21 | ok |
| `Engine.PlayerPawn.UpdateSensitivity` | function | 16 | ok |
| `Engine.PlayerPawn.SloMo` | function | 9 | ok |
| `Engine.PlayerPawn.CheatFlying` | state | 1 | ok |
| `Engine.PlayerPawn.CheatFlying.PlayerTick` | state-function | 19 | ok |
| `Engine.PlayerPawn.CheatFlying.AnimEnd` | state-function | 6 | ok |
| `Engine.PlayerPawn.CheatFlying.ProcessMove` | state-function | 32 | ok |
| `Engine.PlayerPawn.CheatFlying.TakeDamage` | state-function | 2 | ok |
| `Engine.PlayerPawn.CheatFlying.BeginState` | state-function | 24 | ok |
| `Engine.PlayerPawn.CheatFlying.PlayerMove` | state-function | 179 | ok |
| `Engine.PlayerPawn.NormalSpeed` | function | 23 | ok |
| `Engine.PlayerPawn.ServerSetSloMo` | function | 46 | ok |
| `Engine.PlayerPawn.SetFriction` | function | 58 | ok |
| `Engine.PlayerPawn.ShowPath` | function | 76 | ok |
| `Engine.PlayerPawn.PlayerInput` | function | 1281 | ok |
| `Engine.PlayerPawn.MoveWhileCasting` | function | 4 | ok |
| `Engine.PlayerPawn.TurnWhileStrafingMult` | function | 8 | ok |
| `Engine.PlayerPawn.InputIsDisabled` | function | 4 | ok |
| `Engine.PlayerPawn.PlayerSwimming` | state | 1 | ok |
| `Engine.PlayerPawn.PlayerSwimming.BeginState` | state-function | 23 | ok |
| `Engine.PlayerPawn.PlayerSwimming.Timer` | state-function | 46 | ok |
| `Engine.PlayerPawn.PlayerSwimming.AnimEnd` | state-function | 104 | ok |
| `Engine.PlayerPawn.PlayerSwimming.PlayerMove` | state-function | 304 | ok |
| `Engine.PlayerPawn.PlayerSwimming.PlayerTick` | state-function | 19 | ok |
| `Engine.PlayerPawn.PlayerSwimming.ProcessMove` | state-function | 149 | ok |
| `Engine.PlayerPawn.PlayerSwimming.ZoneChange` | state-function | 205 | ok |
| `Engine.PlayerPawn.PlayerSwimming.Landed` | state-function | 62 | ok |
| `Engine.PlayerPawn.PlayerSwimming.UpdateEyeHeight` | state-function | 434 | ok |
| `Engine.PlayerPawn.PlayerIsAiming` | function | 4 | ok |
| `Engine.PlayerPawn.JumpOffPawn` | function | 28 | ok |
| `Engine.PlayerPawn.FeigningDeath` | state | 1 | ok |
| `Engine.PlayerPawn.FeigningDeath.ServerMove` | state-function | 88 | ok |
| `Engine.PlayerPawn.FeigningDeath.AnimEnd` | state-function | 49 | ok |
| `Engine.PlayerPawn.FeigningDeath.PlayChatting` | state-function | 2 | ok |
| `Engine.PlayerPawn.FeigningDeath.AltFire` | state-function | 8 | ok |
| `Engine.PlayerPawn.FeigningDeath.ZoneChange` | state-function | 26 | ok |
| `Engine.PlayerPawn.FeigningDeath.ChangedWeapon` | state-function | 16 | ok |
| `Engine.PlayerPawn.FeigningDeath.EndState` | state-function | 20 | ok |
| `Engine.PlayerPawn.FeigningDeath.BeginState` | state-function | 90 | ok |
| `Engine.PlayerPawn.FeigningDeath.PlayDying` | state-function | 32 | ok |
| `Engine.PlayerPawn.FeigningDeath.PlayTakeHit` | state-function | 26 | ok |
| `Engine.PlayerPawn.FeigningDeath.PlayerMove` | state-function | 184 | ok |
| `Engine.PlayerPawn.FeigningDeath.PlayerTick` | state-function | 23 | ok |
| `Engine.PlayerPawn.FeigningDeath.ProcessMove` | state-function | 52 | ok |
| `Engine.PlayerPawn.FeigningDeath.Rise` | state-function | 59 | ok |
| `Engine.PlayerPawn.FeigningDeath.Landed` | state-function | 56 | ok |
| `Engine.PlayerPawn.FeigningDeath.Taunt` | state-function | 2 | ok |
| `Engine.PlayerPawn.FeigningDeath.Fire` | state-function | 8 | ok |
| `Engine.PlayerPawn.UpdateWeaponPriorities` | function | 59 | ok |
| `Engine.PlayerPawn.Gibbed` | function | 72 | ok |
| `Engine.PlayerPawn.SpawnGibbedCarcass` | function | 53 | ok |
| `Engine.PlayerPawn.PreBeginPlay` | function | 50 | ok |
| `Engine.PlayerPawn.ServerUpdateWeapons` | function | 62 | ok |
| `Engine.PlayerPawn.PlayDodge` | function | 6 | ok |
| `Engine.PlayerPawn.PlayTurning` | function | 2 | ok |
| `Engine.PlayerPawn.PlaySwimming` | function | 6 | ok |
| `Engine.PlayerPawn.PlayFeignDeath` | function | 2 | ok |
| `Engine.PlayerPawn.PlayRising` | function | 2 | ok |
| `Engine.PlayerPawn.PlayerWalking` | state | 1 | ok |
| `Engine.PlayerPawn.PlayerWalking.EndState` | state-function | 25 | ok |
| `Engine.PlayerPawn.PlayerWalking.BeginState` | state-function | 85 | ok |
| `Engine.PlayerPawn.PlayerWalking.ProcessMove` | state-function | 573 | ok |
| `Engine.PlayerPawn.PlayerWalking.PlayerMove` | state-function | 816 | ok |
| `Engine.PlayerPawn.PlayerWalking.PlayerTick` | state-function | 19 | ok |
| `Engine.PlayerPawn.PlayerWalking.Landed` | state-function | 54 | ok |
| `Engine.PlayerPawn.PlayerWalking.Dodge` | state-function | 276 | ok |
| `Engine.PlayerPawn.PlayerWalking.ZoneChange` | state-function | 26 | ok |
| `Engine.PlayerPawn.PlayerWalking.AnimEnd` | state-function | 325 | ok |
| `Engine.PlayerPawn.PlayerWalking.ServerFeignDeath` | state-function | 41 | ok |
| `Engine.PlayerPawn.PlayerWalking.FeignDeath` | state-function | 39 | ok |
| `Engine.PlayerPawn.SwimAnimUpdate` | function | 83 | ok |
| `Engine.PlayerPawn.InvalidState` | state | 1 | ok |
| `Engine.PlayerPawn.InvalidState.PlayerMove` | state-function | 48 | ok |
| `Engine.PlayerPawn.InvalidState.PlayerTick` | state-function | 41 | ok |
| `Engine.PlayerPawn.AdjustAim` | function | 386 | ok |
| `Engine.PlayerPawn.AdjustHitLocation` | function | 193 | ok |
| `Engine.PlayerPawn.ScreenToWorld` | function | 3 | ok |
| `Engine.PlayerPawn.UpdateRotation` | function | 281 | ok |
| `Engine.PlayerPawn.ViewShake` | function | 117 | ok |
| `Engine.PlayerPawn.Falling` | function | 6 | ok |
| `Engine.PlayerPawn.CameraToWorld` | function | 115 | ok |
| `Engine.PlayerPawn.AttitudeTo` | function | 33 | ok |
| `Engine.PlayerPawn.KillMessage` | function | 41 | ok |
| `Engine.PlayerPawn.ViewFlash` | function | 352 | ok |
| `Engine.PlayerPawn.SetViewFlash` | function | 13 | ok |
| `Engine.PlayerPawn.FacingActor` | function | 75 | ok |
| `HGame.CutScene.OpenCutConsole` | function | 35 | ok |
| `HGame.CutScene.CutCue` | function | 55 | ok |
| `HGame.CutScene.OnResolveGameState` | function | 259 | ok |
| `HGame.CutScene.PostBeginPlay` | function | 15 | ok |
| `HGame.CutScene.CreateThreads` | function | 304 | ok |
| `HGame.CutScene.DeleteThreads` | function | 52 | ok |
| `HGame.CutScene.CheckFinished` | function | 77 | ok |
| `HGame.CutScene.disabled` | state | 1 | ok |
| `HGame.CutScene.disabled.BeginState` | state-function | 109 | ok |
| `HGame.CutScene.disabled.Touch` | state-function | 2 | ok |
| `HGame.CutScene.disabled.Trigger` | state-function | 2 | ok |
| `HGame.CutScene.disabled.Play` | state-function | 2 | ok |
| `HGame.CutScene.Idle` | state | 46 | ok |
| `HGame.CutScene.Idle.BeginState` | state-function | 112 | ok |
| `HGame.CutScene.Idle.Touch` | state-function | 70 | ok |
| `HGame.CutScene.Idle.Trigger` | state-function | 151 | ok |
| `HGame.CutScene.Running` | state | 52 | ok |
| `HGame.CutScene.Running.Tick` | state-function | 94 | ok |
| `HGame.CutScene.Finished` | state | 69 | ok |
| `HGame.CutScene.FastForward` | function | 2 | ok |
| `HGame.CutScene.Play` | function | 99 | ok |
| `HGame.CutScene.CutCommand` | function | 73 | ok |
| `HGame.TriggerChangeLevel.Waiting` | state | 1 | ok |
| `HGame.TriggerChangeLevel.Waiting.Trigger` | state-function | 33 | ok |
| `HGame.TriggerChangeLevel.Waiting.Touch` | state-function | 29 | ok |
| `HGame.TriggerChangeLevel.ProcessTrigger` | function | 529 | ok |
| `Engine.Trigger.Touch` | function | 103 | ok |
| `Engine.Trigger.Activate` | function | 164 | ok |
| `Engine.Trigger.UnTouch` | function | 27 | ok |
| `Engine.Trigger.PostBeginPlay` | function | 43 | ok |
| `Engine.Trigger.FindTriggerActor` | function | 85 | ok |
| `Engine.Trigger.SpecialHandling` | function | 327 | ok |
| `Engine.Trigger.CheckTouchList` | function | 48 | ok |
| `Engine.Trigger.GlobalTriggerHandler` | function | 81 | ok |
| `Engine.Trigger.NormalTrigger` | state | 1 | ok |
| `Engine.Trigger.NormalTrigger.Trigger` | state-function | 12 | ok |
| `Engine.Trigger.OtherTriggerToggles` | state | 1 | ok |
| `Engine.Trigger.OtherTriggerToggles.Trigger` | state-function | 34 | ok |
| `Engine.Trigger.OtherTriggerTurnsOn` | state | 1 | ok |
| `Engine.Trigger.OtherTriggerTurnsOn.Trigger` | state-function | 40 | ok |
| `Engine.Trigger.OtherTriggerTurnsOff` | state | 1 | ok |
| `Engine.Trigger.OtherTriggerTurnsOff.Trigger` | state-function | 18 | ok |
| `Engine.Trigger.OtherTriggerDetermines` | state | 1 | ok |
| `Engine.Trigger.OtherTriggerDetermines.Trigger` | state-function | 443 | ok |
| `Engine.Trigger.Deactivate` | function | 42 | ok |
| `Engine.Trigger.IsRelevant` | function | 199 | ok |
| `Engine.Trigger.Timer` | function | 89 | ok |
| `Engine.Trigger.TakeDamage` | function | 114 | ok |
| `Engine.MusicTrigger.Trigger` | function | 132 | ok |
| `HGame.HProp.Touch` | function | 75 | ok |
| `HGame.HProp.PreBeginPlay` | function | 30 | ok |
| `HGame.HProp.HitWall` | function | 91 | ok |
| `HGame.HProp.PickupProp` | state | 286 | ok |
| `HGame.HProp.PickupProp.BeginState` | state-function | 8 | ok |
| `HGame.HProp.PickupProp.Tick` | state-function | 9 | ok |
| `HGame.HProp.BounceIntoPlace` | state | 28 | ok |
| `HGame.HProp.BounceIntoPlace.BeginState` | state-function | 17 | ok |
| `HGame.HProp.BounceIntoPlace.HitWall` | state-function | 92 | ok |
| `HGame.HProp.BounceIntoPlace.Tick` | state-function | 72 | ok |
| `HGame.HProp.RenderHud` | function | 15 | ok |
| `HGame.HProp.ZoomToCamera` | function | 51 | ok |
| `HGame.HProp.DropOffProp` | state | 129 | ok |
| `HGame.HProp.DropOffProp.BeginState` | state-function | 14 | ok |
| `HGame.HProp.DropOffProp.Tick` | state-function | 9 | ok |
| `HGame.HProp.TickPickupOrDropOff` | function | 154 | ok |
| `HGame.HProp.SetFlyProps` | function | 25 | ok |
| `HGame.HProp.DoDropOffProp` | function | 23 | ok |
| `HGame.HProp.DoPickupProp` | function | 7 | ok |
| `HGame.HProp.FlyToNewPosition` | function | 144 | ok |
| `HGame.HProp.FaceCamera` | function | 70 | ok |
| `Engine.PlayerStart.Trigger` | function | 13 | ok |
| `Engine.PlayerStart.PlayTeleportEffect` | function | 95 | ok |
| `Engine.Mover.InterpolateTo` | function | 333 | ok |
| `Engine.Mover.BeginPlay` | function | 178 | ok |
| `Engine.Mover.HandleDoor` | function | 4 | ok |
| `Engine.Mover.InterpolateEnd` | function | 239 | ok |
| `Engine.Mover.DoOpen` | function | 119 | ok |
| `Engine.Mover.DoClose` | function | 176 | ok |
| `Engine.Mover.Bump` | function | 156 | ok |
| `Engine.Mover.Tick` | function | 310 | ok |
| `Engine.Mover.PostBeginPlay` | function | 321 | ok |
| `Engine.Mover.EncroachingOn` | function | 577 | ok |
| `Engine.Mover.MakeGroupReturn` | function | 61 | ok |
| `Engine.Mover.MakeGroupStop` | function | 41 | ok |
| `Engine.Mover.IsRelevant` | function | 117 | ok |
| `Engine.Mover.FinishedOpening` | function | 136 | ok |
| `Engine.Mover.TriggerOpenTimed` | state | 85 | ok |
| `Engine.Mover.TriggerOpenTimed.Trigger` | state-function | 39 | ok |
| `Engine.Mover.TriggerOpenTimed.HandleDoor` | state-function | 10 | ok |
| `Engine.Mover.TriggerOpenTimed.BeginState` | state-function | 8 | ok |
| `Engine.Mover.SetKeyframe` | function | 40 | ok |
| `Engine.Mover.TakeDamage` | function | 35 | ok |
| `Engine.Mover.KeyFrameReached` | function | 7 | ok |
| `Engine.Mover.FinishedClosing` | function | 66 | ok |
| `Engine.Mover.InstantReset` | function | 39 | ok |
| `Engine.Mover.TriggerPound` | state | 88 | ok |
| `Engine.Mover.TriggerPound.BeginState` | state-function | 7 | ok |
| `Engine.Mover.TriggerPound.HandleDoor` | state-function | 10 | ok |
| `Engine.Mover.TriggerPound.Trigger` | state-function | 26 | ok |
| `Engine.Mover.TriggerPound.UnTrigger` | state-function | 36 | ok |
| `Engine.Mover.FinishNotify` | function | 215 | ok |
| `Engine.Mover.TriggerToggle` | state | 78 | ok |
| `Engine.Mover.TriggerToggle.HandleDoor` | state-function | 10 | ok |
| `Engine.Mover.TriggerToggle.Trigger` | state-function | 72 | ok |
| `Engine.Mover.SpecialHandling` | function | 141 | ok |
| `Engine.Mover.TriggerControl` | state | 80 | ok |
| `Engine.Mover.TriggerControl.HandleDoor` | state-function | 10 | ok |
| `Engine.Mover.TriggerControl.Trigger` | state-function | 44 | ok |
| `Engine.Mover.TriggerControl.UnTrigger` | state-function | 50 | ok |
| `Engine.Mover.TriggerControl.BeginState` | state-function | 7 | ok |
| `Engine.Mover.HandleTriggerDoor` | function | 864 | ok |
| `Engine.Mover.FindTriggerActor` | function | 307 | ok |
| `Engine.Mover.Timer` | function | 150 | ok |
| `Engine.Mover.MoverMoves` | function | 52 | ok |
| `Engine.Mover.BumpOpenTimed` | state | 85 | ok |
| `Engine.Mover.BumpOpenTimed.HandleDoor` | state-function | 64 | ok |
| `Engine.Mover.BumpOpenTimed.Bump` | state-function | 40 | ok |
| `Engine.Mover.BumpButton` | state | 85 | ok |
| `Engine.Mover.BumpButton.Bump` | state-function | 42 | ok |
| `Engine.Mover.BumpButton.HandleDoor` | state-function | 41 | ok |
| `Engine.Mover.BumpButton.BeginEvent` | state-function | 8 | ok |
| `Engine.Mover.BumpButton.EndEvent` | state-function | 19 | ok |
| `Engine.Mover.StandOpenTimed` | state | 87 | ok |
| `Engine.Mover.StandOpenTimed.HandleDoor` | state-function | 76 | ok |
| `Engine.Mover.StandOpenTimed.Attach` | state-function | 26 | ok |
| `HGame.harry.AltFire` | function | 120 | ok |
| `HGame.harry.PlayerTick` | function | 364 | ok |
| `HGame.harry.Mount` | function | 61 | ok |
| `HGame.harry.PreBeginPlay` | function | 96 | ok |
| `HGame.harry.PostBeginPlay` | function | 394 | ok |
| `HGame.harry.Possess` | function | 26 | ok |
| `HGame.harry.PlayerInput` | function | 227 | ok |
| `HGame.harry.CutCommand` | function | 764 | ok |
| `HGame.harry.Timer` | function | 6 | ok |
| `HGame.harry.TravelPostAccept` | function | 327 | ok |
| `HGame.harry.Landed` | function | 9 | ok |
| `HGame.harry.TakeDamage` | function | 1014 | ok |
| `HGame.harry.KillHarry` | function | 143 | ok |
| `HGame.harry.Bump` | function | 16 | ok |
| `HGame.harry.PlayerWalking` | state | 1 | ok |
| `HGame.harry.PlayerWalking.EndState` | state-function | 85 | ok |
| `HGame.harry.PlayerWalking.BeginState` | state-function | 104 | ok |
| `HGame.harry.PlayerWalking.Bump` | state-function | 32 | ok |
| `HGame.harry.PlayerWalking.HitWall` | state-function | 33 | ok |
| `HGame.harry.PlayerWalking.PlayerTick` | state-function | 499 | ok |
| `HGame.harry.PlayerWalking.PlayerMove` | state-function | 774 | ok |
| `HGame.harry.PlayerWalking.AnimEnd` | state-function | 225 | ok |
| `HGame.harry.PlayerWalking.ProcessMove` | state-function | 281 | ok |
| `HGame.harry.PlayerWalking.ProcessFalling` | state-function | 184 | ok |
| `HGame.harry.PlayerWalking.Landed` | state-function | 696 | ok |
| `HGame.harry.PlayerWalking.StartAiming` | state-function | 133 | ok |
| `HGame.harry.PlayerWalking.ZoneChange` | state-function | 26 | ok |
| `HGame.harry.PlayerWalking.UnTouch` | state-function | 33 | ok |
| `HGame.harry.PlayerWalking.Touch` | state-function | 32 | ok |
| `HGame.harry.PlayWaiting` | function | 198 | ok |
| `HGame.harry.Fire` | function | 42 | ok |
| `HGame.harry.PlayInAir` | function | 44 | ok |
| `HGame.harry.StartAiming` | function | 2 | ok |
| `HGame.harry.stateCutIdle` | state | 1 | ok |
| `HGame.harry.stateCutIdle.BeginState` | state-function | 58 | ok |
| `HGame.harry.Died` | function | 7 | ok |
| `HGame.harry.DisablePlayerInput` | function | 25 | ok |
| `HGame.harry.EnablePlayerInput` | function | 25 | ok |
| `HGame.harry.InputIsDisabled` | function | 7 | ok |
| `HGame.harry.PlayPeevesHack` | function | 2 | ok |
| `HGame.harry.AddToSpellBook` | function | 62 | ok |
| `HGame.harry.SaveGame` | function | 13 | ok |
| `HGame.harry.UpdateDuelingRanks` | function | 55 | ok |
| `HGame.harry.ConvertGameStateToNumber` | function | 71 | ok |
| `HGame.harry.HaveObjectiveText` | function | 10 | ok |
| `HGame.harry.SetObjectiveTextId` | function | 9 | ok |
| `HGame.harry.HandleSpellDuelExpelliarmus` | function | 4 | ok |
| `HGame.harry.IsInSpellBook` | function | 36 | ok |
| `HGame.harry.HandleSpellDuelMimblewimble` | function | 396 | ok |
| `HGame.harry.HandleSpellDuelRictusempra` | function | 394 | ok |
| `HGame.harry.CheckIfHarryLostDuel` | function | 124 | ok |
| `HGame.harry.AddAllSpellsToSpellBook` | function | 72 | ok |
| `HGame.harry.HandleSpellIncantationSound` | function | 1195 | ok |
| `HGame.harry.AddToSpellBookByString` | function | 262 | ok |
| `HGame.harry.ClearNonTravelStatus` | function | 137 | ok |
| `HGame.harry.ClearSpellBook` | function | 37 | ok |
| `HGame.harry.TurnOnDuelingMode` | function | 256 | ok |
| `HGame.harry.TurnOffDuelingMode` | function | 137 | ok |
| `HGame.harry.HandleDuelPlayerInput` | function | 370 | ok |
| `HGame.harry.CopyCardStatusFromManagerToHarry` | function | 316 | ok |
| `HGame.harry.EctoRefAdd` | function | 47 | ok |
| `HGame.harry.EctoRefSub` | function | 56 | ok |
| `HGame.harry.SleepyAnimTimerAdd` | function | 50 | ok |
| `HGame.harry.CopyGenericStatusFromManagerToHarry` | function | 340 | ok |
| `HGame.harry.CopyAllStatusFromManagerToHarry` | function | 10 | ok |
| `HGame.harry.DoDrinkWiggenwell` | function | 101 | ok |
| `HGame.harry.MoveWhileCasting` | function | 7 | ok |
| `HGame.harry.ToggleUseSword` | function | 48 | ok |
| `HGame.harry.SleepyAnimTimerSub` | function | 38 | ok |
| `HGame.harry.SetMaxSleepyAnim` | function | 10 | ok |
| `HGame.harry.CutQuestion` | function | 849 | ok |
| `HGame.harry.AddHousePoints` | function | 317 | ok |
| `HGame.harry.Add60HousePointsToGryffindor` | function | 43 | ok |
| `HGame.harry.IsEngagedWithVendor` | function | 9 | ok |
| `HGame.harry.EndVendorEngagement` | function | 13 | ok |
| `HGame.harry.StartVendorEngagement` | function | 15 | ok |
| `HGame.harry.SpellLearning` | state | 1 | ok |
| `HGame.harry.SpellLearning.PlayerInput` | state-function | 23 | ok |
| `HGame.harry.SpellLearning.AltFire` | state-function | 2 | ok |
| `HGame.harry.SpellLearning.ProcessMove` | state-function | 2 | ok |
| `HGame.harry.EndSpellLearning` | function | 12 | ok |
| `HGame.harry.StartSpellLearning` | function | 14 | ok |
| `HGame.harry.makeTarget` | function | 156 | ok |
| `HGame.harry.PlayerCalcView` | function | 44 | ok |
| `HGame.harry.harryfrozen` | state | 1 | ok |
| `HGame.harry.harryfrozen.EndState` | state-function | 2 | ok |
| `HGame.harry.harryfrozen.BeginState` | state-function | 2 | ok |
| `HGame.harry.harryfrozen.AltFire` | state-function | 2 | ok |
| `HGame.harry.harryfrozen.Fire` | state-function | 2 | ok |
| `HGame.harry.exittoMenu` | state | 37 | ok |
| `HGame.harry.startmenu` | function | 44 | ok |
| `HGame.harry.HarryIsDead` | function | 10 | ok |
| `HGame.harry.ReceiveIconMessage` | function | 24 | ok |
| `HGame.harry.GetCurrFidgetAnimName` | function | 74 | ok |
| `HGame.harry.GetCurrIdleAnimName` | function | 117 | ok |
| `HGame.harry.waitForDeath` | state | 101 | ok |
| `HGame.harry.displaydemoMessage` | function | 2 | ok |
| `HGame.harry.nailed` | function | 36 | ok |
| `HGame.harry.GetSwordFireTargetLoc` | function | 17 | ok |
| `HGame.harry.TurnWhileStrafingMult` | function | 52 | ok |
| `HGame.harry.WebAnimRefCountAdd` | function | 23 | ok |
| `HGame.harry.WebAnimRefCountSub` | function | 53 | ok |
| `HGame.harry.LeaveEcto` | function | 8 | ok |
| `HGame.harry.DestroyClass` | function | 71 | ok |
| `HGame.harry.ListGroups` | function | 40 | ok |
| `HGame.harry.AdjustAim` | function | 325 | ok |
| `HGame.harry.GameEnded` | state | 1 | ok |
| `HGame.harry.GameEnded.Died` | state-function | 2 | ok |
| `HGame.harry.GameEnded.TakeDamage` | state-function | 2 | ok |
| `HGame.harry.GameEnded.KilledBy` | state-function | 2 | ok |
| `HGame.harry.KeepPawnInsidePlane` | function | 131 | ok |
| `HGame.harry.ProcessAccel` | function | 402 | ok |
| `HGame.harry.PreClientTravel` | function | 410 | ok |
| `HGame.harry.UpdateRotationToTarget` | function | 142 | ok |
| `HGame.harry.StopSpongifyEffects` | function | 42 | ok |
| `HGame.harry.CreateSpongifyEffects` | function | 124 | ok |
| `HGame.harry.SpawnParticles` | function | 25 | ok |
| `HGame.harry.SetNewMesh` | function | 74 | ok |
| `HGame.harry.MyGetAnimGroup` | function | 48 | ok |
| `HGame.harry.PlayingFidgetAnimation` | function | 70 | ok |
| `HGame.harry.PlayingIdleAnimation` | function | 68 | ok |
| `HGame.harry.CurrentAnimHasFootStepSounds` | function | 92 | ok |
| `HGame.harry.CopyAllStatusFromHarryToManager` | function | 10 | ok |
| `HGame.harry.CopyGenericStatusFromHarryToManager` | function | 139 | ok |
| `HGame.harry.DisplayFirstErrorMessages` | function | 313 | ok |
| `HGame.harry.CopyCardCardStatusFromHarryToManager` | function | 262 | ok |
| `HGame.harry.PlayerIsAimingWithCharge` | function | 7 | ok |
| `HGame.harry.PlayerIsAiming` | function | 7 | ok |
| `HGame.harry.TurnOnCastingVars` | function | 105 | ok |
| `HGame.harry.TurnOffSpellCursor` | function | 39 | ok |
| `HGame.harry.TurnOffCastingVars` | function | 14 | ok |
| `HGame.harry.StopAiming` | function | 36 | ok |
| `HGame.harry.StopAimSoundFX` | function | 47 | ok |
| `HGame.harry.StartAimSoundFX` | function | 68 | ok |
| `HGame.harry.CelebrateBronzeCardSet` | state | 248 | ok |
| `HGame.harry.DoCelebrateBronzeCardSet` | function | 36 | ok |
| `HGame.harry.ChessDeath` | state | 22 | ok |
| `HGame.harry.statePickBitOfGoyle` | state | 74 | ok |
| `HGame.harry.MountFinish` | state | 98 | ok |
| `HGame.harry.MountFinish.EndState` | state-function | 36 | ok |
| `HGame.harry.MountFinish.BeginState` | state-function | 49 | ok |
| `HGame.harry.MountFinish.PlayerTick` | state-function | 31 | ok |
| `HGame.harry.MountFinish.ProcessMove` | state-function | 28 | ok |
| `HGame.harry.MountFinish.AltFire` | state-function | 2 | ok |
| `HGame.harry.MountFinish.Mount` | state-function | 2 | ok |
| `HGame.harry.Mounting` | state | 253 | ok |
| `HGame.harry.Mounting.BeginState` | state-function | 49 | ok |
| `HGame.harry.Mounting.ProcessMove` | state-function | 28 | ok |
| `HGame.harry.Mounting.AltFire` | state-function | 2 | ok |
| `HGame.harry.Mounting.Mount` | state-function | 2 | ok |
| `HGame.harry.FallingMount` | state | 46 | ok |
| `HGame.harry.FallingMount.BeginState` | state-function | 26 | ok |
| `HGame.harry.FallingMount.Landed` | state-function | 38 | ok |
| `HGame.harry.FallingMount.Mount` | state-function | 20 | ok |
| `HGame.harry.FallingMount.PlayerTick` | state-function | 153 | ok |
| `HGame.harry.FallingMount.AltFire` | state-function | 2 | ok |
| `HGame.harry.getnumHousePointsHarry` | function | 6 | ok |
| `HGame.harry.getLastHousePointsHarry` | function | 6 | ok |
| `HGame.harry.cast` | function | 444 | ok |
| `HGame.harry.TweenToWaiting` | function | 67 | ok |
| `HGame.harry.PlayIdle` | function | 41 | ok |
| `HGame.harry.PlayCrawling` | function | 17 | ok |
| `HGame.harry.PlayDuck` | function | 22 | ok |
| `HGame.harry.PlayRunning` | function | 11 | ok |
| `HGame.harry.TweenToRunning` | function | 247 | ok |
| `HGame.harry.PlayTurning` | function | 21 | ok |
| `HGame.harry.getNumHousePointsGryffindor` | function | 6 | ok |
| `HGame.harry.PlayLandedSound` | function | 387 | ok |
| `HGame.harry.DoJump` | function | 401 | ok |
| `HGame.harry.PlayHit` | function | 2 | ok |
| `HGame.harry.getNumHousePointsSlytherin` | function | 6 | ok |
| `HGame.harry.getNumHousePointsHufflePuff` | function | 6 | ok |
| `HGame.harry.PlayFootStep` | function | 556 | ok |
| `HGame.harry.Falling` | function | 42 | ok |
| `HGame.harry.Summon` | function | 9 | ok |
| `HGame.harry.getNumHousePointsRavenclaw` | function | 6 | ok |
| `HGame.harry.SaveStateName` | function | 54 | ok |
| `HGame.harry.RestoreStateName` | function | 7 | ok |
| `HGame.harry.HarryKnockBack` | function | 55 | ok |
| `HGame.harry.PreSetMovement` | function | 38 | ok |
| `HGame.harry.TurnDebugModeOn` | function | 24 | ok |
| `HGame.harry.DebugState` | function | 2 | ok |
| `HGame.harry.SetCarryingActor` | function | 198 | ok |
| `HGame.harry.ClientPlaySound` | function | 245 | ok |
| `HGame.harry.InvertBroomPitch` | function | 14 | ok |
| `HGame.harry.InFrontOfHarry` | function | 172 | ok |
| `HGame.harry.HarryAtMapMarker` | function | 183 | ok |
| `HGame.harry.StopBossEncounter` | function | 179 | ok |
| `HGame.harry.StartBossEncounter` | function | 349 | ok |
| `HGame.harry.SpawnAndAttach` | function | 27 | ok |
| `HGame.harry.KeyDownEvent` | function | 519 | ok |
| `HGame.harry.wingspell` | state | 123 | ok |
| `HGame.harry.wingspell.EndState` | state-function | 17 | ok |
| `HGame.harry.wingspell.Fire` | state-function | 35 | ok |
| `HGame.harry.wingspell.AltFire` | state-function | 35 | ok |
| `HGame.harry.wingspell.Tick` | state-function | 2 | ok |
| `HGame.harry.LookAtActor` | state | 47 | ok |
| `HGame.harry.LookAtActor.AltFire` | state-function | 2 | ok |
| `HGame.harry.LookAtActor.Fire` | state-function | 2 | ok |
| `HGame.harry.MovementMode` | function | 48 | ok |
| `HGame.harry.freeHarry` | function | 7 | ok |
| `HGame.harry.forceHarrywing` | function | 19 | ok |
| `HGame.harry.forceHarryLook` | function | 14 | ok |
| `HGame.harry.AddPotionsPoints` | function | 55 | ok |
| `HGame.harry.AddJellyBeansPoints` | function | 71 | ok |
| `HGame.harry.managerStatus_PickupItem` | function | 116 | ok |
| `HGame.harry.PotionsCount` | function | 46 | ok |
| `HGame.harry.JellyBeansCount` | function | 46 | ok |
| `HGame.harry.AddGryffindorPoints` | function | 21 | ok |
| `HGame.harry.GetHealth` | function | 15 | ok |
| `HGame.harry.GetHealthCount` | function | 70 | ok |
| `HGame.harry.AddHealth` | function | 72 | ok |
| `HGame.harry.GetHealthStatusItem` | function | 18 | ok |
| `HGame.harry.stateInactive` | state | 1 | ok |
| `HGame.harry.stateInactive.DoJump` | state-function | 2 | ok |
| `HGame.harry.stateInactive.AltFire` | state-function | 2 | ok |
| `HGame.harry.stateInactive.Fire` | state-function | 2 | ok |
| `HGame.harry.stateInactive.TakeDamage` | state-function | 2 | ok |
| `HGame.harry.stateDead` | state | 152 | ok |
| `HGame.harry.stateDead.FindFaintLocation` | state-function | 321 | ok |
| `HGame.harry.stateDead.BeginState` | state-function | 99 | ok |
| `HGame.harry.stateDead.AltFire` | state-function | 2 | ok |
| `HGame.harry.stateDead.Fire` | state-function | 2 | ok |
| `HGame.harry.KillHarryWithClub` | function | 73 | ok |
| `HGame.harry.FindClosestTargetPoint` | function | 88 | ok |
| `HGame.harry.FindNearestSavePointID` | function | 196 | ok |
| `HGame.harry.GotoShortcut` | function | 114 | ok |
| `HGame.harry.GotoLocation` | function | 32 | ok |
| `HGame.harry.statePotionMixingIdle` | state | 38 | ok |
| `HGame.harry.statePotionMixingStir` | state | 132 | ok |
| `HGame.harry.statePotionMixingStir.EndState` | state-function | 10 | ok |
| `HGame.harry.statePotionMixingBegin` | state | 38 | ok |
| `HGame.harry.statePotionMixingBegin.BeginState` | state-function | 38 | ok |
| `HGame.harry.GetNearestMixingCauldron` | function | 110 | ok |
| `HGame.harry.IsMixingPotion` | function | 31 | ok |
| `HGame.harry.DoPotionMixingEnd` | function | 36 | ok |
| `HGame.harry.DoPotionMixingIdle` | function | 7 | ok |
| `HGame.harry.DoPotionMixingStir` | function | 7 | ok |
| `HGame.harry.DoPotionMixingBegin` | function | 13 | ok |
| `HGame.harry.statePickupItem` | state | 148 | ok |
| `HGame.harry.statePickupItem.BeginState` | state-function | 38 | ok |
| `HGame.harry.DropCarryingActor` | function | 178 | ok |
| `HGame.harry.PickupActor` | function | 109 | ok |
| `HGame.harry.AttachCarryActor` | function | 28 | ok |
| `HGame.harry.ThrowCarryingActor` | function | 175 | ok |
| `HGame.harry.HarryAccurateThrowObject` | function | 100 | ok |
| `HGame.harry.AccurateThrowing` | function | 34 | ok |
| `HGame.SpawnThingy.Trigger` | function | 256 | ok |
| `HGame.Hedwig.Tick` | function | 9 | ok |
| `HGame.Hedwig.PreBeginPlay` | function | 25 | ok |
| `HGame.Hedwig.OnEvent` | function | 17 | ok |
| `HGame.Hedwig.SetMyTimer` | function | 56 | ok |
| `HGame.Hedwig.StopOnSpline` | function | 11 | ok |
| `HGame.Hedwig.ContinueOnSpline` | function | 11 | ok |
| `HGame.Hedwig.GotoNewPath` | function | 157 | ok |
| `HGame.Hedwig.PlayerCutCapture` | function | 7 | ok |
| `HGame.Hedwig.PlayerCutRelease` | function | 6 | ok |
| `HGame.Hedwig.stateIdle` | state | 85 | ok |
| `HGame.Hedwig.CutIdle` | state | 50 | ok |
| `HGame.Hedwig.patrolFollowSpline` | state | 14 | ok |
| `HGame.Hedwig.patrolFollowSpline.EndState` | state-function | 20 | ok |
| `HGame.Hedwig.patrolFollowSpline.Tick` | state-function | 8 | ok |
| `HGame.Hedwig.patrolFollowSpline.Timer` | state-function | 11 | ok |
| `HGame.DestroyTrigger.PassThru` | function | 102 | ok |
| `HGame.DestroyTrigger.Trigger` | function | 76 | ok |
| `HGame.DestroyTrigger.Touch` | function | 82 | ok |
| `HGame.CreatureGenerator.PostBeginPlay` | function | 121 | ok |
| `HGame.CreatureGenerator.Tick` | function | 84 | ok |
| `HGame.CreatureGenerator.OnResolveGameState` | function | 29 | ok |
| `HGame.CreatureGenerator.TooFarFromHarry` | function | 37 | ok |
| `HGame.CreatureGenerator.DestroyCreature` | function | 83 | ok |
| `HGame.CreatureGenerator.GenerateCreature` | function | 690 | ok |
| `HGame.CreatureGenerator.TriggerOpenTimed` | state | 49 | ok |
| `HGame.CreatureGenerator.TriggerOpenTimed.Trigger` | state-function | 10 | ok |
| `HGame.CreatureGenerator.CameraCanSeeYou` | function | 29 | ok |
| `HGame.CreatureGenerator.UpdateCreatureLife` | function | 27 | ok |
| `HGame.CreatureGenerator.UpdateCreaturesLife` | function | 35 | ok |
| `HGame.CreatureGenerator.DestroyCreatures` | function | 32 | ok |
| `Engine.GameInfo.AdminLogin` | function | 146 | ok |
| `Engine.GameInfo.AdminLogout` | function | 232 | ok |
| `Engine.GameInfo.PreBeginPlay` | function | 84 | ok |
| `Engine.GameInfo.PostBeginPlay` | function | 226 | ok |
| `Engine.GameInfo.InitLogging` | function | 399 | ok |
| `Engine.GameInfo.Timer` | function | 7 | ok |
| `Engine.GameInfo.GameEnding` | function | 108 | ok |
| `Engine.GameInfo.InitGameReplicationInfo` | function | 62 | ok |
| `Engine.GameInfo.GetInfo` | function | 145 | ok |
| `Engine.GameInfo.GetServerPort` | function | 50 | ok |
| `Engine.GameInfo.SetPause` | function | 97 | ok |
| `Engine.GameInfo.SetGameSpeed` | function | 42 | ok |
| `Engine.GameInfo.DetailChange` | function | 96 | ok |
| `Engine.GameInfo.IsRelevant` | function | 428 | ok |
| `Engine.GameInfo.GrabOption` | function | 114 | ok |
| `Engine.GameInfo.GetKeyValue` | function | 69 | ok |
| `Engine.GameInfo.HasOption` | function | 46 | ok |
| `Engine.GameInfo.InitGame` | function | 557 | ok |
| `Engine.GameInfo.ProcessServerTravel` | function | 452 | ok |
| `Engine.GameInfo.GetIntOption` | function | 45 | ok |
| `Engine.GameInfo.Login` | function | 1230 | ok |
| `Engine.GameInfo.AddBot` | function | 2 | ok |
| `Engine.GameInfo.ForceAddBot` | function | 2 | ok |
| `Engine.GameInfo.Logout` | function | 187 | ok |
| `Engine.GameInfo.AcceptInventory` | function | 63 | ok |
| `Engine.GameInfo.AddDefaultInventory` | function | 217 | ok |
| `Engine.GameInfo.FindPlayerStart` | function | 238 | ok |
| `Engine.GameInfo.StartPlayer` | function | 95 | ok |
| `Engine.GameInfo.BroadcastRegularDeathMessage` | function | 43 | ok |
| `Engine.GameInfo.ScoreKill` | function | 132 | ok |
| `Engine.GameInfo.KillMessage` | function | 11 | ok |
| `Engine.GameInfo.RegisterMessageMutator` | function | 23 | ok |
| `Engine.GameInfo.ReduceDamage` | function | 31 | ok |
| `Engine.GameInfo.ScoreEvent` | function | 2 | ok |
| `Engine.GameInfo.ShouldRespawn` | function | 54 | ok |
| `Engine.GameInfo.PickupQuery` | function | 76 | ok |
| `Engine.GameInfo.DiscardInventory` | function | 526 | ok |
| `Engine.GameInfo.PlayerJumpZScaling` | function | 8 | ok |
| `Engine.GameInfo.ChangeName` | function | 112 | ok |
| `Engine.GameInfo.PlayerKillMessage` | function | 27 | ok |
| `Engine.GameInfo.CreatureKillMessage` | function | 22 | ok |
| `Engine.GameInfo.SendPlayer` | function | 19 | ok |
| `Engine.GameInfo.PlayTeleportEffect` | function | 2 | ok |
| `Engine.GameInfo.RestartGame` | function | 23 | ok |
| `Engine.GameInfo.AllowsBroadcast` | function | 132 | ok |
| `Engine.GameInfo.EndGame` | function | 189 | ok |
| `Engine.GameInfo.SetEndCams` | function | 79 | ok |
| `Engine.GameInfo.GetRules` | function | 354 | ok |
| `Engine.GameInfo.LogGameParameters` | function | 1039 | ok |
| `Engine.GameInfo.ResetGame` | function | 2 | ok |
| `Engine.GameInfo.ParseOption` | function | 49 | ok |
| `Engine.GameInfo.GetBeaconText` | function | 64 | ok |
| `Engine.GameInfo.AtCapacity` | function | 22 | ok |
| `Engine.GameInfo.PreLogin` | function | 195 | ok |
| `Engine.GameInfo.CheckIPPolicy` | function | 368 | ok |
| `Engine.GameInfo.PostLogin` | function | 384 | ok |
| `Engine.GameInfo.RestartPlayer` | function | 403 | ok |
| `Engine.GameInfo.Killed` | function | 1030 | ok |
| `Engine.GameInfo.ParseKillMessage` | function | 12 | ok |
| `Engine.GameInfo.CanSpectate` | function | 4 | ok |
| `Engine.GameInfo.RegisterDamageMutator` | function | 23 | ok |
| `Engine.GameInfo.ChangeTeam` | function | 67 | ok |
| `Engine.GameInfo.PlaySpawnEffect` | function | 8 | ok |
| `Engine.Mutator.PreBeginPlay` | function | 2 | ok |
| `Engine.Mutator.PostRender` | function | 2 | ok |
| `Engine.Mutator.ModifyPlayer` | function | 25 | ok |
| `Engine.Mutator.HandleRestartGame` | function | 25 | ok |
| `Engine.Mutator.HandleEndGame` | function | 25 | ok |
| `Engine.Mutator.PreventDeath` | function | 37 | ok |
| `Engine.Mutator.ModifyLogin` | function | 31 | ok |
| `Engine.Mutator.ScoreKill` | function | 28 | ok |
| `Engine.Mutator.MutatedDefaultWeapon` | function | 72 | ok |
| `Engine.Mutator.MyDefaultWeapon` | function | 34 | ok |
| `Engine.Mutator.AddMutator` | function | 35 | ok |
| `Engine.Mutator.MutatorTeamMessage` | function | 47 | ok |
| `Engine.Mutator.MutatorBroadcastMessage` | function | 44 | ok |
| `Engine.Mutator.MutatorBroadcastLocalizedMessage` | function | 49 | ok |
| `Engine.Mutator.RegisterHUDMutator` | function | 97 | ok |
| `Engine.Mutator.HandlePickupQuery` | function | 34 | ok |
| `Engine.Mutator.ReplaceWith` | function | 383 | ok |
| `Engine.Mutator.AlwaysKeep` | function | 28 | ok |
| `Engine.Mutator.IsRelevant` | function | 61 | ok |
| `Engine.Mutator.CheckReplacement` | function | 4 | ok |
| `Engine.Mutator.Mutate` | function | 28 | ok |
| `Engine.Mutator.MutatorTakeDamage` | function | 40 | ok |
| `Engine.ActorShadow.AttachToSurface` | function | 2 | ok |
| `Engine.ActorShadow.Tick` | function | 43 | ok |
| `Engine.ActorShadow.Update` | function | 347 | ok |
| `Engine.Decal.Update` | function | 2 | ok |
| `Engine.Decal.AttachToSurface` | function | 20 | ok |
| `Engine.Decal.AttachDecal` | function | 6 | ok |
| `Engine.Decal.PostBeginPlay` | function | 6 | ok |
| `Engine.Decal.Destroyed` | function | 10 | ok |
| `Engine.PlayerReplicationInfo.PostBeginPlay` | function | 44 | ok |
| `Engine.PlayerReplicationInfo.Timer` | function | 220 | ok |
| `HGame.SpellCursor.TurnTargetingOn` | function | 2 | ok |
| `HGame.SpellCursor.SetDebugMode` | function | 11 | ok |
| `HGame.SpellCursor.IsLockedOn` | function | 9 | ok |
| `HGame.SpellCursor.Destroyed` | function | 25 | ok |
| `HGame.SpellCursor.SetLOSDistance` | function | 79 | ok |
| `HGame.SpellCursor.TurnOnSpellGestureFX` | function | 125 | ok |
| `HGame.SpellCursor.TurnTargetingOff` | function | 10 | ok |
| `HGame.SpellCursor.PreBeginPlay` | function | 111 | ok |
| `HGame.SpellCursor.GetGestureTexture` | function | 80 | ok |
| `HGame.SpellCursor.CanCameraSeeYouInFOV` | function | 160 | ok |
| `HGame.SpellCursor.UpdateCursor` | function | 810 | ok |
| `HGame.SpellCursor.LookForTarget` | function | 35 | ok |
| `HGame.SpellCursor.LockOn` | function | 766 | ok |
| `HGame.SpellCursor.UnLock` | function | 40 | ok |
| `HGame.SpellCursor.StartLockedOnSoundLoop` | function | 18 | ok |
| `HGame.SpellCursor.StopLockedOnSoundLoop` | function | 10 | ok |
| `HGame.SpellCursor.TurnSparklesOff` | function | 2 | ok |
| `HGame.SpellCursor.SetSparklesIdle` | function | 206 | ok |
| `HGame.SpellCursor.SetSparklesSeeking` | function | 277 | ok |
| `HGame.SpellCursor.SetSparklesLockedOn` | function | 380 | ok |
| `HGame.SpellCursor.stateIdle` | state | 55 | ok |
| `HGame.SpellCursor.stateIdle.Tick` | state-function | 102 | ok |
| `HGame.SpellCursor.stateIdle.BeginState` | state-function | 29 | ok |
| `HGame.SpellCursor.stateIdle.TurnTargetingOn` | state-function | 7 | ok |
| `HGame.SpellCursor.stateSeeking` | state | 59 | ok |
| `HGame.SpellCursor.stateSeeking.Tick` | state-function | 117 | ok |
| `HGame.SpellCursor.stateSeeking.BeginState` | state-function | 20 | ok |
| `HGame.SpellCursor.stateSeeking.EndState` | state-function | 2 | ok |
| `HGame.SpellCursor.stateLockedOn` | state | 73 | ok |
| `HGame.SpellCursor.stateLockedOn.Tick` | state-function | 483 | ok |
| `HGame.GestureSprite.PreBeginPlay` | function | 17 | ok |
| `HGame.StatusManager.PreBeginPlay` | function | 21 | ok |
| `HGame.StatusManager.RenderHudItemManager` | function | 89 | ok |
| `HGame.StatusManager.PickupItem` | function | 210 | ok |
| `HGame.StatusManager.DropOffItem` | function | 98 | ok |
| `HGame.StatusManager.IncrementCount` | function | 39 | ok |
| `HGame.StatusManager.SetCount` | function | 38 | ok |
| `HGame.StatusManager.IncrementCountPotential` | function | 39 | ok |
| `HGame.StatusManager.GetHudLocation` | function | 158 | ok |
| `HGame.StatusManager.GetStatusItem` | function | 93 | ok |
| `HGame.StatusManager.GetStatusGroup` | function | 233 | ok |
| `HGame.StatusManager.CreateStartupItems` | function | 80 | ok |
| `HGame.StatusManager.AddHPointsG` | function | 12 | ok |
| `HGame.StatusManager.GetHPointsG` | function | 18 | ok |
| `HGame.StatusManager.AddHPointsH` | function | 12 | ok |
| `HGame.StatusManager.GetHPointsH` | function | 18 | ok |
| `HGame.StatusManager.AddHPointsS` | function | 12 | ok |
| `HGame.StatusManager.GetHPointsS` | function | 18 | ok |
| `HGame.StatusManager.AddHPointsR` | function | 12 | ok |
| `HGame.StatusManager.GetHPointsR` | function | 18 | ok |
| `HGame.StatusManager.AddHousePoints` | function | 261 | ok |
| `HGame.StatusManager.AddFMucus` | function | 15 | ok |
| `HGame.StatusManager.GetFMucusCount` | function | 18 | ok |
| `HGame.StatusManager.AddWBark` | function | 15 | ok |
| `HGame.StatusManager.GetWBarkCount` | function | 18 | ok |
| `HGame.StatusManager.AddBicorn` | function | 15 | ok |
| `HGame.StatusManager.GetBicornCount` | function | 18 | ok |
| `HGame.StatusManager.AddBoomslang` | function | 15 | ok |
| `HGame.StatusManager.GetBoomslangCount` | function | 18 | ok |
| `HGame.StatusManager.AddBeans` | function | 15 | ok |
| `HGame.StatusManager.GetBeanCount` | function | 18 | ok |
| `HGame.StatusManager.addpotions` | function | 15 | ok |
| `HGame.StatusManager.GetPotionCount` | function | 18 | ok |
| `HGame.StatusManager.AddHealth` | function | 15 | ok |
| `HGame.StatusManager.GetHealthCount` | function | 18 | ok |
| `HGame.StatusManager.SetHealthCount` | function | 15 | ok |
| `HGame.StatusManager.AddHealthPotential` | function | 15 | ok |
| `HGame.StatusManager.GetHealthPotentialCount` | function | 19 | ok |
| `HGame.StatusManager.GiveCardToHarry` | function | 10 | ok |
| `HGame.StatusManager.GiveCardToVendors` | function | 10 | ok |
| `HGame.StatusManager.GiveCard` | function | 319 | ok |
| `HGame.StatusManager.GiveAllCardsToHarry` | function | 307 | ok |
| `HGame.StatusManager.ShowCardData` | function | 25 | ok |
| `HGame.HudItemManager.RenderHudItemManager` | function | 2 | ok |
| `HGame.HudItemManager.GetScaleFactor` | function | 21 | ok |
| `HGame.StatusGroupHealth.GetGroupFinalXY_2` | function | 12 | ok |
| `HGame.StatusGroupHealth.GetGroupFlyOriginXY` | function | 41 | ok |
| `HGame.StatusGroup.GetGroupFlyOriginXY` | function | 87 | ok |
| `HGame.StatusGroup.GetGroupFinalXY_2` | function | 85 | ok |
| `HGame.StatusGroup.GetGroupFinalXY` | function | 42 | ok |
| `HGame.StatusGroup.OnCountIncremented` | function | 2 | ok |
| `HGame.StatusGroup.GetFadeValue` | function | 5 | ok |
| `HGame.StatusGroup.GetGroupCurrXY` | function | 25 | ok |
| `HGame.StatusGroup.RenderHudItemManager` | function | 419 | ok |
| `HGame.StatusGroup.SetEffectTypeToPermanent` | function | 22 | ok |
| `HGame.StatusGroup.SetEffectTypeToNormal` | function | 13 | ok |
| `HGame.StatusGroup.SetCutSceneRenderModeToNormal` | function | 11 | ok |
| `HGame.StatusGroup.SetCutSceneRenderMode` | function | 11 | ok |
| `HGame.StatusGroup.GetStatusItem` | function | 218 | ok |
| `HGame.StatusGroup.HandleMenuModeSwitching` | function | 81 | ok |
| `HGame.StatusGroup.GetItemPosition` | function | 332 | ok |
| `HGame.StatusGroup.GetItemLocation` | function | 228 | ok |
| `HGame.StatusGroup.GetDrawColor` | function | 47 | ok |
| `HGame.StatusGroup.GetGroupCurrXY_2` | function | 28 | ok |
| `HGame.StatusGroup.IncrementCount` | function | 43 | ok |
| `HGame.StatusGroup.IncrementCountPotential` | function | 43 | ok |
| `HGame.StatusGroup.GetTimeRatio` | function | 36 | ok |
| `HGame.StatusGroup.CalcFadeValue` | function | 220 | ok |
| `HGame.StatusGroup.CalcFlyXY` | function | 320 | ok |
| `HGame.StatusGroup.GetScaleFactor` | function | 21 | ok |
| `HGame.StatusGroup.PreBeginPlay` | function | 9 | ok |
| `HGame.StatusGroup.Idle` | state | 1 | ok |
| `HGame.StatusGroup.Idle.OnCountIncremented` | state-function | 41 | ok |
| `HGame.StatusGroup.Idle.RenderHudItemManager` | state-function | 17 | ok |
| `HGame.StatusGroup.Idle.BeginState` | state-function | 33 | ok |
| `HGame.StatusGroup.EffectIn` | state | 1 | ok |
| `HGame.StatusGroup.EffectIn.Tick` | state-function | 53 | ok |
| `HGame.StatusGroup.EffectIn.GetFadeValue` | state-function | 32 | ok |
| `HGame.StatusGroup.EffectIn.GetGroupCurrXY` | state-function | 70 | ok |
| `HGame.StatusGroup.EffectIn.BeginState` | state-function | 9 | ok |
| `HGame.StatusGroup.Hold` | state | 1 | ok |
| `HGame.StatusGroup.Hold.Timer` | state-function | 16 | ok |
| `HGame.StatusGroup.Hold.OnCountIncremented` | state-function | 44 | ok |
| `HGame.StatusGroup.Hold.BeginState` | state-function | 35 | ok |
| `HGame.StatusGroup.EffectOut` | state | 1 | ok |
| `HGame.StatusGroup.EffectOut.Tick` | state-function | 52 | ok |
| `HGame.StatusGroup.EffectOut.OnCountIncremented` | state-function | 33 | ok |
| `HGame.StatusGroup.EffectOut.GetFadeValue` | state-function | 32 | ok |
| `HGame.StatusGroup.EffectOut.GetGroupCurrXY` | state-function | 70 | ok |
| `HGame.StatusGroup.EffectOut.BeginState` | state-function | 9 | ok |
| `HGame.StatusItemHealth.GetChangeInHealthDrawColor` | function | 33 | ok |
| `HGame.StatusItemHealth.GetHealthDrawColor` | function | 36 | ok |
| `HGame.StatusItemHealth.PreBeginPlay` | function | 143 | ok |
| `HGame.StatusItemHealth.IncrementCount` | function | 21 | ok |
| `HGame.StatusItemHealth.IncrementCountPotential` | function | 24 | ok |
| `HGame.StatusItemHealth.DrawItem` | function | 674 | ok |
| `HGame.StatusItemHealth.NormalDisplay` | state | 1 | ok |
| `HGame.StatusItemHealth.NormalDisplay.BeginState` | state-function | 7 | ok |
| `HGame.StatusItemHealth.HoldChange` | state | 1 | ok |
| `HGame.StatusItemHealth.HoldChange.Timer` | state-function | 7 | ok |
| `HGame.StatusItemHealth.HoldChange.GetHealthDrawColor` | state-function | 65 | ok |
| `HGame.StatusItemHealth.HoldChange.GetChangeInHealthDrawColor` | state-function | 36 | ok |
| `HGame.StatusItemHealth.HoldChange.GetHoldChangeTime` | state-function | 56 | ok |
| `HGame.StatusItemHealth.HoldChange.BeginState` | state-function | 31 | ok |
| `HGame.StatusItemHealth.FadeChangeOut` | state | 1 | ok |
| `HGame.StatusItemHealth.FadeChangeOut.Tick` | state-function | 44 | ok |
| `HGame.StatusItemHealth.FadeChangeOut.GetHealthDrawColor` | state-function | 101 | ok |
| `HGame.StatusItemHealth.FadeChangeOut.GetChangeInHealthDrawColor` | state-function | 63 | ok |
| `HGame.StatusItemHealth.FadeChangeOut.GetFadeChangeTime` | state-function | 56 | ok |
| `HGame.StatusItemHealth.FadeChangeOut.BeginState` | state-function | 39 | ok |
| `HGame.StatusItem.IncrementCountPotential` | function | 56 | ok |
| `HGame.StatusItem.IncrementCount` | function | 13 | ok |
| `HGame.StatusItem.PreBeginPlay` | function | 26 | ok |
| `HGame.StatusItem.DrawItem` | function | 61 | ok |
| `HGame.StatusItem.SetCount` | function | 76 | ok |
| `HGame.StatusItem.GetCount` | function | 5 | ok |
| `HGame.StatusItem.GetPotentialCount` | function | 6 | ok |
| `HGame.StatusItem.SetCountToMaxPotential` | function | 8 | ok |
| `HGame.StatusItem.GetPotentialToMaxCountRatio` | function | 46 | ok |
| `HGame.StatusItem.GetCountToMaxCountRatio` | function | 45 | ok |
| `HGame.StatusItem.GetCountToCurrPotentialRatio` | function | 12 | ok |
| `HGame.StatusItem.GetCountColor` | function | 125 | ok |
| `HGame.StatusItem.GetCountFont` | function | 184 | ok |
| `HGame.StatusItem.DrawCount` | function | 235 | ok |
| `HGame.StatusItem.GetHudIconUSize` | function | 29 | ok |
| `HGame.StatusItem.GetHudIconVSize` | function | 29 | ok |
| `HGame.StatusItem.GetToolTip` | function | 22 | ok |
| `HGame.StatusGroupHousePoints.PostBeginPlay` | function | 19 | ok |
| `HGame.StatusGroupHousePoints.GetGroupFinalXY` | function | 42 | ok |
| `HGame.StatusGroupHousePoints.GetGroupFinalXY_2` | function | 56 | ok |
| `HGame.StatusGroupHousePoints.GetGroupFlyOriginXY` | function | 66 | ok |
| `HGame.StatusGroupHousePoints.TransitionUpdateHousepoints` | function | 582 | ok |
| `HGame.StatusGroupHousePoints.QuidditchUpdateHousepoints` | function | 491 | ok |
| `HGame.StatusGroupHousePoints.CutCommand` | function | 124 | ok |
| `HGame.StatusGroupHousePoints.CutQuestion` | function | 166 | ok |
| `HGame.StatusGroupHousePoints.ResolveTies` | function | 90 | ok |
| `HGame.StatusGroupHousePoints.AdjustIfTie` | function | 205 | ok |
| `HGame.StatusGroupHousePoints.IsHouseAhead` | function | 155 | ok |
| `HGame.StatusGroupJellybeans.GetGroupFinalXY` | function | 42 | ok |
| `HGame.StatusGroupJellybeans.GetGroupFinalXY_2` | function | 39 | ok |
| `HGame.StatusGroupJellybeans.GetGroupFlyOriginXY` | function | 66 | ok |
| `HGame.StatusGroupWizardCards.GetGroupFinalXY` | function | 42 | ok |
| `HGame.StatusGroupWizardCards.GetGroupFinalXY_2` | function | 39 | ok |
| `HGame.StatusGroupWizardCards.GetGroupFlyOriginXY` | function | 66 | ok |
| `HGame.StatusGroupWizardCards.AssignVendorCards` | function | 360 | ok |
| `HGame.StatusGroupWizardCards.AssignVendorCardFromClass` | function | 214 | ok |
| `HGame.StatusGroupWizardCards.RemoveHarryOwnedCardsFromLevel` | function | 530 | ok |
| `HGame.StatusGroupWizardCards.GetGameStateTokenIndex` | function | 85 | ok |
| `HGame.StatusGroupWizardCards.HasCardGameStatePassed` | function | 173 | ok |
| `HGame.StatusGroupWizardCards.ShowCardData` | function | 50 | ok |
| `HGame.StatusGroupWizardCards.SetLastObtainedCardItem` | function | 74 | ok |
| `HGame.StatusGroupWizardCards.GetLastObtainedCardType` | function | 6 | ok |
| `HGame.StatusGroupWizardCards.SetLastObtainedCardTypeAsInt` | function | 59 | ok |
| `HGame.StatusGroupWizardCards.GetLastObtainedCardTypeAsInt` | function | 40 | ok |
| `HGame.StatusItemWizardCards.UpdateCount` | function | 73 | ok |
| `HGame.StatusItemWizardCards.GetCardOwner` | function | 141 | ok |
| `HGame.StatusItemWizardCards.IsOwnedByHarry` | function | 16 | ok |
| `HGame.StatusItemWizardCards.IsOwnedByVendor` | function | 16 | ok |
| `HGame.StatusItemWizardCards.IsOwnedByNone` | function | 16 | ok |
| `HGame.StatusItemWizardCards.SetCardOwner` | function | 223 | ok |
| `HGame.StatusItemWizardCards.GetFirstVendorCardId` | function | 90 | ok |
| `HGame.StatusItemWizardCards.GetFirstVendorCardIdAndClass` | function | 34 | ok |
| `HGame.StatusItemWizardCards.GetCardClassFromId` | function | 1446 | ok |
| `HGame.StatusItemWizardCards.VerifyCardClass` | function | 220 | ok |
| `HGame.StatusItemWizardCards.GetCardData` | function | 138 | ok |
| `HGame.StatusItemWizardCards.GetCardId` | function | 19 | ok |
| `HGame.StatusItemWizardCards.SetCardData` | function | 250 | ok |
| `HGame.StatusItemWizardCards.ShowCardData` | function | 157 | ok |
| `HGame.StatusItemSilverCards.UpdateCount` | function | 196 | ok |
| `Engine.cEyeBlinkAnimChannel.stateBlink` | state | 120 | ok |
| `HGame.BaseCam.ConvertRotToDeg` | function | 30 | ok |
| `HGame.BaseCam.ConvertDegToRot` | function | 22 | ok |
| `HGame.BaseCam.SetMinPitch` | function | 9 | ok |
| `HGame.BaseCam.SetRotStepPitch` | function | 13 | ok |
| `HGame.BaseCam.SetRotTightness` | function | 11 | ok |
| `HGame.BaseCam.SetRotSpeed` | function | 11 | ok |
| `HGame.BaseCam.SetModeByString` | function | 13 | ok |
| `HGame.BaseCam.SetSyncPosWithTarget` | function | 11 | ok |
| `HGame.BaseCam.SetFOV` | function | 64 | ok |
| `HGame.BaseCam.SetPosition` | function | 104 | ok |
| `HGame.BaseCam.GetModeFromString` | function | 163 | ok |
| `HGame.BaseCam.SetCameraMode` | function | 395 | ok |
| `HGame.BaseCam.LoadUserSettings` | function | 146 | ok |
| `HGame.BaseCam.InitRotation` | function | 90 | ok |
| `HGame.BaseCam.InitPosition` | function | 26 | ok |
| `HGame.BaseCam.InitTarget` | function | 30 | ok |
| `HGame.BaseCam.InitPositionAndRotation` | function | 136 | ok |
| `HGame.BaseCam.UpdateRotationUsingVectors` | function | 150 | ok |
| `HGame.BaseCam.ApplyMouseXToDestYaw` | function | 166 | ok |
| `HGame.BaseCam.ApplyMouseYToDestPitch` | function | 249 | ok |
| `HGame.BaseCam.UpdateRotation` | function | 130 | ok |
| `HGame.BaseCam.SetFinalRotation` | function | 41 | ok |
| `HGame.BaseCam.UpdatePosition` | function | 123 | ok |
| `HGame.BaseCam.CheckCollisionWithWorld` | function | 387 | ok |
| `HGame.BaseCam.StateStartup` | state | 41 | ok |
| `HGame.BaseCam.StateStartup.BeginState` | state-function | 2 | ok |
| `HGame.BaseCam.stateIdle` | state | 1 | ok |
| `HGame.BaseCam.StateStandardCam` | state | 14 | ok |
| `HGame.BaseCam.StateStandardCam.TakeDamage` | state-function | 2 | ok |
| `HGame.BaseCam.StateStandardCam.KilledBy` | state-function | 2 | ok |
| `HGame.BaseCam.StateStandardCam.WarnTarget` | state-function | 2 | ok |
| `HGame.BaseCam.StateStandardCam.Died` | state-function | 2 | ok |
| `HGame.BaseCam.StateStandardCam.BeginState` | state-function | 70 | ok |
| `HGame.BaseCam.StateStandardCam.EndState` | state-function | 9 | ok |
| `HGame.BaseCam.StateStandardCam.Tick` | state-function | 50 | ok |
| `HGame.BaseCam.StateStandardCam.LongFall` | state-function | 2 | ok |
| `HGame.BaseCam.StateQuidditchCam` | state | 1 | ok |
| `HGame.BaseCam.StateQuidditchCam.BeginState` | state-function | 71 | ok |
| `HGame.BaseCam.StateQuidditchCam.Tick` | state-function | 115 | ok |
| `HGame.BaseCam.StateFlyingCarCam` | state | 1 | ok |
| `HGame.BaseCam.StateFlyingCarCam.BeginState` | state-function | 71 | ok |
| `HGame.BaseCam.StateFlyingCarCam.Tick` | state-function | 84 | ok |
| `HGame.BaseCam.StateDuelingCam` | state | 14 | ok |
| `HGame.BaseCam.StateDuelingCam.WarnTarget` | state-function | 2 | ok |
| `HGame.BaseCam.StateDuelingCam.Died` | state-function | 2 | ok |
| `HGame.BaseCam.StateDuelingCam.LongFall` | state-function | 2 | ok |
| `HGame.BaseCam.StateDuelingCam.BeginState` | state-function | 70 | ok |
| `HGame.BaseCam.StateDuelingCam.Tick` | state-function | 36 | ok |
| `HGame.BaseCam.StateDuelingCam.TakeDamage` | state-function | 2 | ok |
| `HGame.BaseCam.StateDuelingCam.KilledBy` | state-function | 2 | ok |
| `HGame.BaseCam.StateCutSceneCam` | state | 1 | ok |
| `HGame.BaseCam.StateCutSceneCam.BeginState` | state-function | 176 | ok |
| `HGame.BaseCam.StateCutSceneCam.Tick` | state-function | 42 | ok |
| `HGame.BaseCam.CutCommand_ProcessFlash` | function | 521 | ok |
| `HGame.BaseCam.CutCommand_ProcessShake` | function | 230 | ok |
| `HGame.BaseCam.CutCommand_ProcessFade` | function | 500 | ok |
| `HGame.BaseCam.SetYaw` | function | 12 | ok |
| `HGame.BaseCam.SetPitch` | function | 13 | ok |
| `HGame.BaseCam.SetRoll` | function | 13 | ok |
| `HGame.BaseCam.SetMaxPitch` | function | 9 | ok |
| `HGame.BaseCam.SetRotStep` | function | 9 | ok |
| `HGame.BaseCam.SetRotStepYaw` | function | 12 | ok |
| `HGame.BaseCam.SetRotStepRoll` | function | 13 | ok |
| `HGame.BaseCam.SetMoveTightness` | function | 11 | ok |
| `HGame.BaseCam.SetMoveSpeed` | function | 11 | ok |
| `HGame.BaseCam.SetDistance` | function | 11 | ok |
| `HGame.BaseCam.SetTargetActor` | function | 15 | ok |
| `HGame.BaseCam.SetOffset` | function | 15 | ok |
| `HGame.BaseCam.SetYOffset` | function | 15 | ok |
| `HGame.BaseCam.SetXOffset` | function | 15 | ok |
| `HGame.BaseCam.SetZOffset` | function | 15 | ok |
| `HGame.BaseCam.SetSyncRotWithTarget` | function | 11 | ok |
| `HGame.BaseCam.TransitionToCameraMode` | function | 14 | ok |
| `HGame.BaseCam.ShowSettings` | function | 1123 | ok |
| `HGame.BaseCam.SaveUserSettings` | function | 145 | ok |
| `HGame.BaseCam.PreBeginPlay` | function | 41 | ok |
| `HGame.BaseCam.PostBeginPlay` | function | 250 | ok |
| `HGame.BaseCam.InitSettings` | function | 76 | ok |
| `HGame.BaseCam.UpdateDistanceScalar` | function | 144 | ok |
| `HGame.BaseCam.SetDestRotation` | function | 9 | ok |
| `HGame.BaseCam.StateTransition` | state | 1 | ok |
| `HGame.BaseCam.StateTransition.Tick` | state-function | 180 | ok |
| `HGame.BaseCam.StateTransition.BeginState` | state-function | 176 | ok |
| `HGame.BaseCam.StateBossCam` | state | 1 | ok |
| `HGame.BaseCam.StateBossCam.BeginState` | state-function | 66 | ok |
| `HGame.BaseCam.StateBossCam.Tick` | state-function | 132 | ok |
| `HGame.BaseCam.StateFreeCam` | state | 14 | ok |
| `HGame.BaseCam.StateFreeCam.KilledBy` | state-function | 2 | ok |
| `HGame.BaseCam.StateFreeCam.WarnTarget` | state-function | 2 | ok |
| `HGame.BaseCam.StateFreeCam.Died` | state-function | 2 | ok |
| `HGame.BaseCam.StateFreeCam.BeginState` | state-function | 96 | ok |
| `HGame.BaseCam.StateFreeCam.Tick` | state-function | 903 | ok |
| `HGame.BaseCam.StateFreeCam.TakeDamage` | state-function | 2 | ok |
| `HGame.BaseCam.StateFreeCam.LongFall` | state-function | 2 | ok |
| `HGame.BaseCam.CutCommand` | function | 700 | ok |
| `HGame.BaseCam.CutCommand_ProcessFOV` | function | 231 | ok |
| `HGame.BaseCam.CutCommand_ProcessLocked` | function | 505 | ok |
| `HGame.BaseCam.CutCommand_ProcessTarget` | function | 604 | ok |
| `HGame.BaseCam.CameraCanSeeYou` | function | 91 | ok |
| `HGame.BaseCamTarget.UpdateOrientation` | function | 110 | ok |
| `HGame.BaseCamTarget.Tick` | function | 12 | ok |
| `HGame.BaseCamTarget.SetNewRotation` | function | 73 | ok |
| `HGame.BaseCamTarget.SetAttachedToByCutName` | function | 225 | ok |
| `HGame.BaseCamTarget.SetYOffset` | function | 16 | ok |
| `HGame.BaseCamTarget.SetZOffset` | function | 15 | ok |
| `HGame.BaseCamTarget.SetAttachedTo` | function | 20 | ok |
| `HGame.BaseCamTarget.SetAttachedToByName` | function | 317 | ok |
| `HGame.BaseCamTarget.SetOffset` | function | 13 | ok |
| `HGame.BaseCamTarget.SetXOffset` | function | 16 | ok |
| `HGame.BaseCamTarget.IsAttached` | function | 9 | ok |
| `HGame.cHarryAnimChannel.CanPickSomethingUp` | function | 4 | ok |
| `HGame.cHarryAnimChannel.PlayHarryMovementAnims` | function | 4 | ok |
| `HGame.cHarryAnimChannel.stateReactMimbleWimble` | state | 73 | ok |
| `HGame.cHarryAnimChannel.stateReactMimbleWimble.BeginState` | state-function | 16 | ok |
| `HGame.cHarryAnimChannel.DoReactMimbleWimble` | function | 18 | ok |
| `HGame.cHarryAnimChannel.stateReactRictusempra` | state | 73 | ok |
| `HGame.cHarryAnimChannel.stateReactRictusempra.BeginState` | state-function | 16 | ok |
| `HGame.cHarryAnimChannel.stateDrinkWiggenwell` | state | 246 | ok |
| `HGame.cHarryAnimChannel.stateDrinkWiggenwell.EndState` | state-function | 96 | ok |
| `HGame.cHarryAnimChannel.stateDrinkWiggenwell.BeginState` | state-function | 16 | ok |
| `HGame.cHarryAnimChannel.DoDrinkWiggenwell` | function | 18 | ok |
| `HGame.cHarryAnimChannel.stateEctoJump` | state | 95 | ok |
| `HGame.cHarryAnimChannel.stateEctoJump.BeginState` | state-function | 16 | ok |
| `HGame.cHarryAnimChannel.stateEctoJump.PlayHarryMovementAnims` | state-function | 4 | ok |
| `HGame.cHarryAnimChannel.stateEctoJump.CanPickSomethingUp` | state-function | 4 | ok |
| `HGame.cHarryAnimChannel.DoEctoJump` | function | 18 | ok |
| `HGame.cHarryAnimChannel.DoKnockBack` | function | 31 | ok |
| `HGame.cHarryAnimChannel.stateCancelCasting` | state | 117 | ok |
| `HGame.cHarryAnimChannel.stateThrow` | state | 73 | ok |
| `HGame.cHarryAnimChannel.stateThrow.CanPickSomethingUp` | state-function | 4 | ok |
| `HGame.cHarryAnimChannel.DoReactRictusempra` | function | 18 | ok |
| `HGame.cHarryAnimChannel.stateWebJump` | state | 95 | ok |
| `HGame.cHarryAnimChannel.stateWebJump.PlayHarryMovementAnims` | state-function | 4 | ok |
| `HGame.cHarryAnimChannel.stateWebJump.BeginState` | state-function | 16 | ok |
| `HGame.cHarryAnimChannel.stateWebJump.CanPickSomethingUp` | state-function | 4 | ok |
| `HGame.cHarryAnimChannel.DoWebJump` | function | 18 | ok |
| `HGame.cHarryAnimChannel.stateSleepyJump` | state | 95 | ok |
| `HGame.cHarryAnimChannel.stateSleepyJump.BeginState` | state-function | 16 | ok |
| `HGame.cHarryAnimChannel.stateSleepyJump.CanPickSomethingUp` | state-function | 4 | ok |
| `HGame.cHarryAnimChannel.stateSleepyJump.PlayHarryMovementAnims` | state-function | 4 | ok |
| `HGame.cHarryAnimChannel.DoSleepyJump` | function | 18 | ok |
| `HGame.cHarryAnimChannel.GotoStateCasting` | function | 7 | ok |
| `HGame.cHarryAnimChannel.GotoStateThrow` | function | 7 | ok |
| `HGame.cHarryAnimChannel.stateKnockBack` | state | 163 | ok |
| `HGame.cHarryAnimChannel.stateKnockBack.BeginState` | state-function | 16 | ok |
| `HGame.cHarryAnimChannel.stateKnockBack.CanPickSomethingUp` | state-function | 4 | ok |
| `HGame.cHarryAnimChannel.stateCast` | state | 30 | ok |
| `HGame.cHarryAnimChannel.stateCast.BeginState` | state-function | 52 | ok |
| `HGame.cHarryAnimChannel.stateDefenceCast` | state | 188 | ok |
| `HGame.cHarryAnimChannel.stateDuelingCast` | state | 124 | ok |
| `HGame.cHarryAnimChannel.stateCasting` | state | 112 | ok |
| `HGame.cHarryAnimChannel.statePickupItem` | state | 75 | ok |
| `HGame.cHarryAnimChannel.statePickupItem.CanPickSomethingUp` | state-function | 4 | ok |
| `HGame.cHarryAnimChannel.statePickupItem.BeginState` | state-function | 18 | ok |
| `HGame.cHarryAnimChannel.stateIdle` | state | 1 | ok |
| `HGame.cHarryAnimChannel.stateIdle.BeginState` | state-function | 14 | ok |
| `HGame.cHarryAnimChannel.cast` | function | 14 | ok |
| `HGame.cHarryAnimChannel.IsCarryingActor` | function | 29 | ok |

## Opcode histogram

| opcode | count |
|---|---:|
| EndFunctionParms | 10440 |
| InstanceVariable | 8276 |
| NumberedNative | 6822 |
| LocalVariable | 6669 |
| Let | 3469 |
| Context | 3393 |
| JumpIfNot | 2742 |
| Return | 2540 |
| VirtualFunction | 2178 |
| Nothing | 2105 |
| BoolVariable | 1679 |
| Jump | 1327 |
| FloatConst | 1218 |
| StringConst | 1212 |
| ExtendedNativeHi | 1209 |
| StructMember | 1061 |
| NoObject | 883 |
| Skip | 744 |
| IntConstByte | 732 |
| ByteConst | 728 |
| NameConst | 700 |
| ObjectConst | 669 |
| ByteToInt | 659 |
| LetBool | 633 |
| False | 620 |
| True | 595 |
| IntZero | 567 |
| NativeParm | 516 |
| IntToFloat | 511 |
| DynamicCast | 454 |
| Case | 417 |
| ArrayElement | 293 |
| SelfToken | 274 |
| FinalFunction | 202 |
| IntOne | 176 |
| IntConst | 174 |
| VectorConst | 141 |
| FloatToInt | 133 |
| Stop | 133 |
| IteratorPop | 127 |
| IteratorNext | 125 |
| Iterator | 118 |
| ObjectToString | 105 |
| LabelTable | 75 |
| IntToString | 69 |
| DefaultVariable | 65 |
| IntToByte | 65 |
| NameToString | 50 |
| Switch | 49 |
| StringToFloat | 47 |
| GlobalFunction | 29 |
| VectorToRotator | 29 |
| RotatorToVector | 27 |
| FloatToString | 26 |
| RotationConst | 26 |
| ByteToFloat | 23 |
| ClassContext | 22 |
| GotoLabel | 17 |
| BoolToString | 16 |
| EatString | 14 |
| StringToName | 12 |
| VectorToString | 12 |
| FloatToByte | 8 |
| MetaCast | 8 |
| StringToInt | 8 |
| RotatorToString | 6 |
| ByteToString | 5 |
| Assert | 1 |
| StringToBool | 1 |

## Named functions

| function | calls |
|---|---:|
| += | 181 |
| << | 91 |
| Subsystem | 54 |
| SimpleCommandlet | 35 |
| < | 34 |
| SHEER_XZ | 28 |
| SHEER_YZ | 28 |
| EndState | 24 |
| LessEqual_IntInt | 23 |
| PlaneV | 23 |
| * | 21 |
| <final-ref:-58> | 19 |
| @ | 19 |
| -= | 15 |
| Core | 15 |
| IsServer | 15 |
| <final-ref:-99> | 14 |
| HelloWorldCommandlet | 14 |
| Right | 14 |
| EqualEqual_FloatFloat | 13 |
| ReturnValue | 13 |
| <final-ref:-37> | 12 |
| <final-ref:-60> | 12 |
| Rand | 12 |
| Roll | 12 |
| VSize2D | 12 |
| ** | 11 |
| Atan | 11 |
| Log | 11 |
| MirrorVectorByNormal | 11 |
| StaticSaveConfig | 11 |
| <final-ref:-38> | 10 |
| EqualEqual_NameName | 10 |
| HelpParm | 10 |
| SubtractEqual_RotatorRotator | 10 |
| UClamp | 10 |
| ~ | 9 |
| <final-ref:118> | 8 |
| BoundingVolume | 8 |
| Cross | 8 |
| Display | 8 |
| Dot_VectorVector | 8 |
| EqualEqual_ObjectObject | 8 |
| Filter | 8 |
| GetPropertyText | 8 |
| IntProperty | 8 |
| NotEqual_IntInt | 8 |
| NotEqual_VectorVector | 8 |
| SwitchToBestWeapon | 8 |
| Default__Subsystem | 7 |
| Enum | 7 |
| FMax | 7 |
| GameSpeed | 7 |
| GetUnAxes | 7 |
| Greater_IntInt | 7 |
| HandlePickupQuery | 7 |
| ImpactSoundSet | 7 |
| Locale | 7 |
| NotEqual_RotatorRotator | 7 |
| PlaneC | 7 |
| Subtract_PreInt | 7 |
| TweenToFighter | 7 |
| ZoneInfo | 7 |
| % | 6 |
| ClientUpdatePosition | 6 |
| Commandlet | 6 |
| CreatureSoundGroup | 6 |
| Cross_VectorVector | 6 |
| Died | 6 |
| Divide_IntInt | 6 |
| FRand | 6 |
| Fire | 6 |
| G | 6 |
| GetItemName | 6 |
| GetTimeStamp | 6 |
| GreaterEqual_FloatFloat | 6 |
| GreaterEqual_IntInt | 6 |
| InStr | 6 |
| Less_IntInt | 6 |
| Multiply_FloatFloat | 6 |
| Multiply_VectorVector | 6 |
| Normalize | 6 |
| SHEER_ZY | 6 |
| Subtract_PreFloat | 6 |
| Vect | 6 |
| bHidden | 6 |
| || | 6 |
| $ | 5 |
| <final-ref:-77> | 5 |
| <final-ref:829> | 5 |
| Add_IntInt | 5 |
| Class | 5 |
| ClientReStart | 5 |
| EqualEqual_VectorVector | 5 |
| IsValid | 5 |
| Landed | 5 |
| LazyLoad | 5 |
| LessEqual_StrStr | 5 |
| MultiplyEqual_FloatFloat | 5 |
| MultiplyEqual_VectorVector | 5 |
| MultiplyMultiply_FloatFloat | 5 |
| Normal | 5 |
| NotEqual_BoolBool | 5 |
| PlayWaiting | 5 |
| RenderOverlays | 5 |
| SaveConfig | 5 |
| Subtract_PlanePlane | 5 |
| Subtract_PreVector | 5 |
| Subtract_VectorVector | 5 |
| TestState | 5 |
| USize | 5 |
| & | 4 |
| <= | 4 |
| <final-ref:-193> | 4 |
| <final-ref:46> | 4 |
| >>> | 4 |
| Abs | 4 |
| ActionDone | 4 |
| AddInventory | 4 |
| Begin | 4 |
| Caps | 4 |
| CheckVisibility | 4 |
| ClientPlaySound | 4 |
| Divide_FloatFloat | 4 |
| Dodge | 4 |
| DynamicLoadObject | 4 |
| ESheerAxis | 4 |
| Enable | 4 |
| GiveAmmo | 4 |
| KeyName | 4 |
| Killed | 4 |
| Less_FloatFloat | 4 |
| LiftCenter | 4 |
| LightColor | 4 |
| MaxInt | 4 |
| MipZero | 4 |
| Origin | 4 |
| RF_Public | 4 |
| RF_Transactional | 4 |
| RotatingMover | 4 |
| SetDefaultDisplayProperties | 4 |
| SoundVolume | 4 |
| SubtractEqual_PlanePlane | 4 |
| Subtract_RotatorRotator | 4 |
| Tag | 4 |
| Touch | 4 |
| Y | 4 |
| YAxis | 4 |
| ZoneChange | 4 |
| ^ | 4 |
| && | 3 |
| <final-ref:-131> | 3 |
| <final-ref:-148> | 3 |
| <final-ref:-269> | 3 |
| <final-ref:-44> | 3 |
| <final-ref:1493> | 3 |
| <final-ref:228> | 3 |
| <final-ref:39> | 3 |
| <final-ref:69> | 3 |
| <final-ref:787> | 3 |
| Actor | 3 |
| AdjustAim | 3 |
| All | 3 |
| Audio | 3 |
| BotDesireability | 3 |
| BroadcastLocalizedMessage | 3 |
| C | 3 |
| ClassProperty | 3 |
| ClientFire | 3 |
| CutCommand_Animate | 3 |
| CutCue | 3 |
| Default__HelloWorldCommandlet | 3 |
| E | 3 |
| Exp | 3 |
| FloatParams | 3 |
| FloatProperty | 3 |
| GetLogFileName | 3 |
| GetServerPort | 3 |
| GlobalTriggerHandler | 3 |
| GreaterEqual_StrStr | 3 |
| GreaterGreaterGreater_IntInt | 3 |
| HandleDoor | 3 |
| Initfor | 3 |
| InterpolateEnd | 3 |
| KilledBy | 3 |
| LogEventString | 3 |
| Loge | 3 |
| MayFail | 3 |
| ModifyLogin | 3 |
| Multiply_IntInt | 3 |
| PawnAtInterpolationPoint | 3 |
| PlaneF | 3 |
| PlayDying | 3 |
| PlayInAir | 3 |
| PlayRecoil | 3 |
| PostRender | 3 |
| PrioritizeArmor | 3 |
| PropName | 3 |
| RateSelf | 3 |
| RecurseTest | 3 |
| ResetConfig | 3 |
| RotRand | 3 |
| ScriptText | 3 |
| Select | 3 |
| ServerReStartPlayer | 3 |
| SetEndCams | 3 |
| SetFOVAngle | 3 |
| SetHand | 3 |
| SetSwitchPriority | 3 |
| SpawnCopy | 3 |
| SwitchInfo | 3 |
| Talk | 3 |
| Texture | 3 |
| TriggerPound | 3 |
| W | 3 |
| XAxis | 3 |
| Z | 3 |
| bNetOptional | 3 |
| bRoll | 3 |
| f | 3 |
| i | 3 |
| intparm | 3 |
| patrolFollowSpline | 3 |
| textstring | 3 |
| <final-ref:-377> | 2 |
| <final-ref:-379> | 2 |
| <final-ref:-384> | 2 |
| <final-ref:-53> | 2 |
| <final-ref:-54> | 2 |
| <final-ref:1032> | 2 |
| <final-ref:1745> | 2 |
| <final-ref:1844> | 2 |
| <final-ref:1922> | 2 |
| <final-ref:1971> | 2 |
| <final-ref:2018> | 2 |
| <final-ref:2020> | 2 |
| <final-ref:2066> | 2 |
| <final-ref:328> | 2 |
| <final-ref:395> | 2 |
| <final-ref:406> | 2 |
| AI | 2 |
| AddAdd_Int | 2 |
| AdjustHitLocation | 2 |
| AltFiring | 2 |
| AssertTriggerOpenTimed | 2 |
| BecomeItem | 2 |
| BecomePickup | 2 |
| BecomeViewTarget | 2 |
| BroadcastMessage | 2 |
| BroadcastRegularDeathMessage | 2 |
| ChangeAlwaysMouseLook | 2 |
| ChangeFrame | 2 |
| ChangeTeam | 2 |
| Clamp | 2 |
| ClientVoiceMessage | 2 |
| Collision | 2 |
| Concat_StrStr | 2 |
| Counter | 2 |
| CreateHeadLookAnimChannel | 2 |
| CutCommand_Talk | 2 |
| Decoration | 2 |
| Default__Locale | 2 |
| Default__SimpleCommandlet | 2 |
| Default__Time | 2 |
| DirAnimInfo | 2 |
| DoTurnTo | 2 |
| DrawText | 2 |
| DrawType | 2 |
| DropDecoration | 2 |
| Dying | 2 |
| Effects | 2 |
| ElevatorTrigger | 2 |
| EndEvent | 2 |
| Engine | 2 |
| FindTriggerActor | 2 |
| FovModifier | 2 |
| Gain | 2 |
| GameReplicationInfo | 2 |
| Generate | 2 |
| GradualTriggerPound | 2 |
| GreaterGreater_VectorRotator | 2 |
| HandleTriggerDoor | 2 |
| HasAnim | 2 |
| IK_A | 2 |
| IK_F19 | 2 |
| IK_F20 | 2 |
| IK_F22 | 2 |
| IK_F4 | 2 |
| IK_GreyMinus | 2 |
| IK_Help | 2 |
| IK_Home | 2 |
| IK_Joy13 | 2 |
| IK_Joy15 | 2 |
| IK_JoyU | 2 |
| IK_Left | 2 |
| IK_MouseW | 2 |
| IK_NumPad1 | 2 |
| IK_PrintScrn | 2 |
| IK_RControl | 2 |
| IK_Separator | 2 |
| IK_Slash | 2 |
| IK_Unknown1C | 2 |
| IK_Unknown3C | 2 |
| IK_Unknown5F | 2 |
| IK_UnknownAA | 2 |
| IK_UnknownEA | 2 |
| IK_UnknownF4 | 2 |
| Idle | 2 |
| InitLogging | 2 |
| InterpolationPoint | 2 |
| Inventory | 2 |
| LightRadius | 2 |
| LogGameEnd | 2 |
| LogPlayerConnect | 2 |
| LogTeamChange | 2 |
| LookInDirection | 2 |
| Max | 2 |
| Menuing | 2 |
| MixMover | 2 |
| MoveKeyframe | 2 |
| Mover | 2 |
| MultiplyEqual_RotatorFloat | 2 |
| MultiplyEqual_VectorFloat | 2 |
| Multiply_PlaneFloat | 2 |
| NameProperty | 2 |
| NavigationPoint | 2 |
| NetPriority | 2 |
| ObjectFlags | 2 |
| ObjectName | 2 |
| OnEvent | 2 |
| Or_IntInt | 2 |
| PS_Appearance | 2 |
| PS_Movement | 2 |
| PainTimer | 2 |
| PlayModifySound | 2 |
| PlayerTick | 2 |
| PlayerTimeOut | 2 |
| PreBeginPlay | 2 |
| PrintTimeDemoResult | 2 |
| ProbeFunc | 2 |
| PropValue | 2 |
| PutDown | 2 |
| R | 2 |
| RF_Transient | 2 |
| ReduceDamage | 2 |
| SHEER_XY | 2 |
| SaveTimeDemo | 2 |
| Say | 2 |
| ScreenFlashScale | 2 |
| ServerChangeSkin | 2 |
| ServerFeignDeath | 2 |
| ServerReStartGame | 2 |
| SetMultiSkin | 2 |
| ShowErrorCount | 2 |
| Smerp | 2 |
| Sounds | 2 |
| SplineTick | 2 |
| Square | 2 |
| SubTestOptionalOut | 2 |
| SubtractEqual_FloatFloat | 2 |
| SubtractEqual_IntInt | 2 |
| Suicided | 2 |
| TT_ClassProximity | 2 |
| TT_PawnProximity | 2 |
| Taunt | 2 |
| TestClass | 2 |
| Thumbnail | 2 |
| UpdateEyeHeight | 2 |
| UpdateRotation | 2 |
| V | 2 |
| VClamp | 2 |
| VSize | 2 |
| Waiting | 2 |
| WarpZoneInfo | 2 |
| ZAxis | 2 |
| ZoneLight | 2 |
| bBlockActors | 2 |
| bCollideWorld | 2 |
| damageAttitudeTo | 2 |
| p | 2 |
| patrol | 2 |
| shot | 2 |
| stateSplinePause | 2 |
| t | 2 |
| <final-ref:-105> | 1 |
| <final-ref:-1274> | 1 |
| <final-ref:-164> | 1 |
| <final-ref:-182> | 1 |
| <final-ref:-243> | 1 |
| <final-ref:-273> | 1 |
| <final-ref:-304> | 1 |
| <final-ref:-62> | 1 |
| <final-ref:-631> | 1 |
| <final-ref:-632> | 1 |
| <final-ref:-63> | 1 |
| <final-ref:-699> | 1 |
| <final-ref:-722> | 1 |
| <final-ref:-731> | 1 |
| <final-ref:-755> | 1 |
| <final-ref:-757> | 1 |
| <final-ref:-760> | 1 |
| <final-ref:-766> | 1 |
| <final-ref:101> | 1 |
| <final-ref:1529> | 1 |
| <final-ref:153> | 1 |
| <final-ref:1549> | 1 |
| <final-ref:1561> | 1 |
| <final-ref:1594> | 1 |
| <final-ref:1598> | 1 |
| <final-ref:1661> | 1 |
| <final-ref:172> | 1 |
| <final-ref:1837> | 1 |
| <final-ref:1843> | 1 |
| <final-ref:2001> | 1 |
| <final-ref:2166> | 1 |
| <final-ref:2355> | 1 |
| <final-ref:2557> | 1 |
| <final-ref:2616> | 1 |
| <final-ref:2642> | 1 |
| <final-ref:3079> | 1 |
| <final-ref:3144> | 1 |
| <final-ref:3509> | 1 |
| <final-ref:3510> | 1 |
| <final-ref:4025> | 1 |
| <final-ref:4058> | 1 |
| <final-ref:407> | 1 |
| <final-ref:4595> | 1 |
| <final-ref:4605> | 1 |
| <final-ref:4607> | 1 |
| <final-ref:4608> | 1 |
| <final-ref:61> | 1 |
| <final-ref:868> | 1 |
| <final-ref:941> | 1 |
| <final-ref:993> | 1 |
| ATTITUDE_Follow | 1 |
| ATTITUDE_Frenzy | 1 |
| ATTITUDE_Ignore | 1 |
| AT_Combine | 1 |
| Activate | 1 |
| AddVelocity | 1 |
| Add_PlanePlane | 1 |
| AirControl | 1 |
| AltFire | 1 |
| AltRefireRate | 1 |
| AlterDestination | 1 |
| AmbientSound | 1 |
| AmbushPoint | 1 |
| Ammo | 1 |
| AnimSequence | 1 |
| ArmorAbsorbDamage | 1 |
| Asc | 1 |
| AssertMover | 1 |
| AtCapacity | 1 |
| AttachToOwner | 1 |
| AvgText | 1 |
| B | 1 |
| BT_AnyBump | 1 |
| BT_PlayerBump | 1 |
| BasedActors | 1 |
| BeginEvent | 1 |
| BigFont | 1 |
| BlockAll | 1 |
| BoundingBox | 1 |
| BringUp | 1 |
| Brush | 1 |
| CCAA | 1 |
| CalcVelocity | 1 |
| CanSpectate | 1 |
| Carcass | 1 |
| CdTrack | 1 |
| CheatFlying | 1 |
| CheckFutureSight | 1 |
| CheckReplacement | 1 |
| ClassIsChildOf | 1 |
| ClearPaths | 1 |
| ClientHearSound | 1 |
| ClientInitialize | 1 |
| ClientMessage | 1 |
| ClientSetLocation | 1 |
| CompressAccel | 1 |
| ConnectingMessage | 1 |
| CreatureKillMessage | 1 |
| CriticalEvent | 1 |
| Crushed | 1 |
| CutCommand_FollowSpline | 1 |
| CutCommand_PlayFacialAnim | 1 |
| CutCommand_TurnTo | 1 |
| CutoffHz | 1 |
| DT_None | 1 |
| DamageScaling | 1 |
| DeActivated | 1 |
| Decapitated | 1 |
| DecayTime | 1 |
| DefaultPlayerState | 1 |
| DeliverLocalizedDialog | 1 |
| Destroyed | 1 |
| Difficulty | 1 |
| Dispatcher | 1 |
| DivideEqual_ByteByte | 1 |
| DivideEqual_VectorFloat | 1 |
| Divide_RotatorFloat | 1 |
| Divide_VectorFloat | 1 |
| DoCutCueNotify | 1 |
| DoJump | 1 |
| DoRunTo | 1 |
| DoWalkTo | 1 |
| DoingTalk | 1 |
| DrawClippedActor | 1 |
| DrawLevelInfo | 1 |
| DrawPattern | 1 |
| DrawSingleView | 1 |
| Drivers | 1 |
| DropFrom | 1 |
| EAdjustJump | 1 |
| EEDDAA | 1 |
| ElevatorMover | 1 |
| EnabledString | 1 |
| EndMenuing | 1 |
| EndZoom | 1 |
| EnvironmentDiffusion | 1 |
| EqualEqual_RotatorRotator | 1 |
| Eradicated | 1 |
| Error | 1 |
| Extent | 1 |
| FClamp | 1 |
| FOOTSTEP_grass | 1 |
| FOOTSTEP_metal | 1 |
| FindGoodView | 1 |
| FindInventoryType | 1 |
| FindPath | 1 |
| FindPlayerStart | 1 |
| FindRandomDest | 1 |
| Finish | 1 |
| FinishedInterpolation | 1 |
| ForceAddBot | 1 |
| ForceFire | 1 |
| ForceGenerate | 1 |
| FormatFloat | 1 |
| FrameRateText | 1 |
| GainedChild | 1 |
| GameCreatorURL | 1 |
| Gasp | 1 |
| GetFontSize | 1 |
| GetHumanName | 1 |
| GetMultiSkin | 1 |
| GetSoundDuration | 1 |
| Gibbed | 1 |
| GiveTo | 1 |
| HUD | 1 |
| HeadZoneChange | 1 |
| HurtRadiusCallBack | 1 |
| IK_B | 1 |
| IK_Comma | 1 |
| IK_D | 1 |
| IK_F | 1 |
| IK_F18 | 1 |
| IK_F2 | 1 |
| IK_H | 1 |
| IK_I | 1 |
| IK_Joy9 | 1 |
| IK_JoyR | 1 |
| IK_JoyV | 1 |
| IK_JoyX | 1 |
| IK_JoyY | 1 |
| IK_MouseZ | 1 |
| IK_Tilde | 1 |
| IK_Unknown17 | 1 |
| IK_Unknown5D | 1 |
| IK_Unknown89 | 1 |
| IK_UnknownDF | 1 |
| IK_UnknownF2 | 1 |
| Init | 1 |
| InitGame | 1 |
| InstantMove | 1 |
| InventoryCapsFloat | 1 |
| KeyEvent | 1 |
| KeyMenuing | 1 |
| KeyType | 1 |
| KillMessage | 1 |
| LEVACT_Connecting | 1 |
| LEVACT_Saving | 1 |
| LE_SlowWave | 1 |
| LE_TorchWaver | 1 |
| LODSet | 1 |
| LT_TexturePaletteLoop | 1 |
| Label | 1 |
| LargeFont | 1 |
| LastSecText | 1 |
| LevelSummary | 1 |
| LightCone | 1 |
| LightSaturation | 1 |
| LoadingMessage | 1 |
| Localize | 1 |
| LocalizedMessage | 1 |
| LogMapParameters | 1 |
| LogPickup | 1 |
| LogPings | 1 |
| LogSuicide | 1 |
| Logout | 1 |
| LookAtPosition | 1 |
| LookInDirectionOfRotator | 1 |
| Look_DownL90 | 1 |
| Look_Forward | 1 |
| Look_UpL45 | 1 |
| Loop | 1 |
| LoopAnim | 1 |
| ME_IgnoreWhenEncroach | 1 |
| ME_ReallyCrushWhenEncroach | 1 |
| MV_GlideByTime | 1 |
| MV_SpringByTime | 1 |
| MakeHeadLookAtPosition | 1 |
| MakeHeadLookInDirectionOfVector | 1 |
| MaxEyeBlinkPeriod | 1 |
| MaxLightingPolyCount | 1 |
| MenuName | 1 |
| MenuTick | 1 |
| MenuTyping | 1 |
| MessagingSpectator | 1 |
| MinLightingPolyCount | 1 |
| MinText | 1 |
| ModifySound | 1 |
| MoveAutonomous | 1 |
| MoverSounds | 1 |
| Multiply_VectorFloat | 1 |
| MusicEventBoost | 1 |
| Mutator | 1 |
| MutatorTeamMessage | 1 |
| MyAutoAim | 1 |
| NetUpdateFrequency | 1 |
| NormalFire | 1 |
| ObjectDetailVeryHigh | 1 |
| Open | 1 |
| OtherTriggerTurnsOff | 1 |
| PHYS_Swimming | 1 |
| ParticleFX | 1 |
| PathNode | 1 |
| PausedMessage | 1 |
| Percent_FloatFloat | 1 |
| Physics | 1 |
| PickupFunction | 1 |
| PickupMessage | 1 |
| Pitch | 1 |
| PlayBeepSound | 1 |
| PlayChatting | 1 |
| PlayGutHit | 1 |
| PlayHeadDeath | 1 |
| PlayIdleAnim | 1 |
| PlayLanded | 1 |
| PlayPostSelect | 1 |
| PlayRunning | 1 |
| PlaySound | 1 |
| PlayTakeHit | 1 |
| PlayTeleportEffect | 1 |
| PlayerKillMessage | 1 |
| PlayerSwimming | 1 |
| PlusDir | 1 |
| Possess | 1 |
| PrecachingMessage | 1 |
| PrintActionMessage | 1 |
| ProcessMenuEscape | 1 |
| ProcessMenuInput | 1 |
| ProcessMenuUpdate | 1 |
| ProcessServerTravel | 1 |
| ProjectileSpeed | 1 |
| Quality | 1 |
| RaiseUp | 1 |
| RecommendWeapon | 1 |
| RefireRate | 1 |
| RemoteRole | 1 |
| RemovePawn | 1 |
| Reset | 1 |
| RestartPlayer | 1 |
| ReverbDelay | 1 |
| RotationRate | 1 |
| RunAnimName | 1 |
| SLOT_None | 1 |
| SPELL_DuelExpelliarmus | 1 |
| SPELL_Ectomatic | 1 |
| SPELL_PetrificusTotalus | 1 |
| STY_Normal | 1 |
| SavingMessage | 1 |
| ScoreKill | 1 |
| SelectNext | 1 |
| Selection | 1 |
| ServerSetHandedness | 1 |
| ServerTaunt | 1 |
| SetBase | 1 |
| SetGameSpeed | 1 |
| SetMsgTick | 1 |
| SetPause | 1 |
| SetPropertyText | 1 |
| SetRespawn | 1 |
| SetSkinElement | 1 |
| SetSubtitleText | 1 |
| ShakeMag | 1 |
| ShakeTime | 1 |
| ShakeView | 1 |
| ShowMenu | 1 |
| SightRadius | 1 |
| Sleep | 1 |
| Sleeping | 1 |
| Sound | 1 |
| Sound_FX | 1 |
| SpecialEvent | 1 |
| SpecialHandling | 1 |
| StartLog | 1 |
| StartWalk | 1 |
| StartZoom | 1 |
| StatLog | 1 |
| StrafeTo | 1 |
| SuggestDefenseStyle | 1 |
| Suicide | 1 |
| SwitchWeapon | 1 |
| TEXF_DXT1 | 1 |
| TEXF_RGB32 | 1 |
| TeamMessage | 1 |
| TeamSay | 1 |
| TestInfo | 1 |
| TextSize | 1 |
| TimeDemoRender | 1 |
| TouchingActors | 1 |
| Trace | 1 |
| TraceActors | 1 |
| TriggerLight | 1 |
| TriggerTurnsOn | 1 |
| TweenTime | 1 |
| TweenToFalling | 1 |
| UnPossess | 1 |
| UnTouch | 1 |
| UnderLift | 1 |
| UpdateWeaponPriorities | 1 |
| Vec | 1 |
| ViewClass | 1 |
| ViewShake | 1 |
| Viewport | 1 |
| VoicePack | 1 |
| WaitForLanding | 1 |
| Warn | 1 |
| WatchActor | 1 |
| WatchPosition | 1 |
| WatchingTarget | 1 |
| WeaponChange | 1 |
| WhiteColor | 1 |
| WorldStandard | 1 |
| Yaw | 1 |
| ZoneActors | 1 |
| actorReachable | 1 |
| angry | 1 |
| bDecayTimeScale | 1 |
| bDifficulty0 | 1 |
| bDirectional | 1 |
| bIsMover | 1 |
| bMoveProjectiles | 1 |
| bNetTemporary | 1 |
| bRotatingPickup | 1 |
| bSinglePlayerStart | 1 |
| bSpecialLit | 1 |
| bStatic | 1 |
| bTrue1 | 1 |
| bTrue2 | 1 |
| bTwoSidedBias | 1 |
| calm | 1 |
| cm | 1 |
| dropped | 1 |
| firm | 1 |
| fpsText | 1 |
| j | 1 |
| stateTurningTo | 1 |
| surprised | 1 |
| sxx | 1 |

## Numbered native slots

| slot | calls | owner(s) |
|---|---:|---|
| 112 (0x0070) | 585 | Core.Object.Concat_StrStr |
| 113 (0x0071) | 195 | Core.Object.GotoState |
| 114 (0x0072) | 279 | Core.Object.EqualEqual_ObjectObject |
| 117 (0x0075) | 11 | Core.Object.Enable |
| 118 (0x0076) | 10 | Core.Object.Disable |
| 119 (0x0077) | 500 | Core.Object.NotEqual_ObjectObject |
| 122 (0x007a) | 56 | Core.Object.EqualEqual_StrStr |
| 123 (0x007b) | 51 | Core.Object.NotEqual_StrStr |
| 124 (0x007c) | 229 | Core.Object.ComplementEqual_StrStr |
| 125 (0x007d) | 49 | Core.Object.Len |
| 126 (0x007e) | 29 | Core.Object.InStr |
| 127 (0x007f) | 67 | Core.Object.Mid |
| 128 (0x0080) | 75 | Core.Object.Left |
| 129 (0x0081) | 305 | Core.Object.Not_PreBool |
| 130 (0x0082) | 480 | Core.Object.AndAnd_BoolBool |
| 131 (0x0083) | 4 | Core.Object.XorXor_BoolBool |
| 132 (0x0084) | 264 | Core.Object.OrOr_BoolBool |
| 139 (0x008b) | 1 | Core.Object.AddAdd_Byte |
| 142 (0x008e) | 2 | Core.Object.EqualEqual_RotatorRotator |
| 143 (0x008f) | 10 | Core.Object.Subtract_PreInt |
| 144 (0x0090) | 16 | Core.Object.Multiply_IntInt |
| 145 (0x0091) | 12 | Core.Object.Divide_IntInt |
| 146 (0x0092) | 50 | Core.Object.Add_IntInt |
| 147 (0x0093) | 61 | Core.Object.Subtract_IntInt |
| 148 (0x0094) | 3 | Core.Object.LessLess_IntInt |
| 149 (0x0095) | 1 | Core.Object.GreaterGreater_IntInt |
| 150 (0x0096) | 157 | Core.Object.Less_IntInt |
| 151 (0x0097) | 107 | Core.Object.Greater_IntInt |
| 152 (0x0098) | 30 | Core.Object.LessEqual_IntInt |
| 153 (0x0099) | 29 | Core.Object.GreaterEqual_IntInt |
| 154 (0x009a) | 275 | Core.Object.EqualEqual_IntInt |
| 155 (0x009b) | 103 | Core.Object.NotEqual_IntInt |
| 156 (0x009c) | 38 | Core.Object.And_IntInt |
| 159 (0x009f) | 1 | Core.Object.MultiplyEqual_IntFloat |
| 161 (0x00a1) | 36 | Core.Object.AddEqual_IntInt |
| 162 (0x00a2) | 6 | Core.Object.SubtractEqual_IntInt |
| 163 (0x00a3) | 13 | Core.Object.AddAdd_PreInt |
| 164 (0x00a4) | 4 | Core.Object.SubtractSubtract_PreInt |
| 165 (0x00a5) | 122 | Core.Object.AddAdd_Int |
| 166 (0x00a6) | 11 | Core.Object.SubtractSubtract_Int |
| 167 (0x00a7) | 42 | Core.Object.Rand |
| 168 (0x00a8) | 37 | Core.Object.At_StrStr |
| 169 (0x00a9) | 23 | Core.Object.Subtract_PreFloat |
| 171 (0x00ab) | 342 | Core.Object.Multiply_FloatFloat |
| 172 (0x00ac) | 96 | Core.Object.Divide_FloatFloat |
| 173 (0x00ad) | 1 | Core.Object.Percent_FloatFloat |
| 174 (0x00ae) | 111 | Core.Object.Add_FloatFloat |
| 175 (0x00af) | 141 | Core.Object.Subtract_FloatFloat |
| 176 (0x00b0) | 141 | Core.Object.Less_FloatFloat |
| 177 (0x00b1) | 173 | Core.Object.Greater_FloatFloat |
| 178 (0x00b2) | 27 | Core.Object.LessEqual_FloatFloat |
| 179 (0x00b3) | 19 | Core.Object.GreaterEqual_FloatFloat |
| 180 (0x00b4) | 24 | Core.Object.EqualEqual_FloatFloat |
| 181 (0x00b5) | 41 | Core.Object.NotEqual_FloatFloat |
| 182 (0x00b6) | 60 | Core.Object.MultiplyEqual_FloatFloat |
| 184 (0x00b8) | 59 | Core.Object.AddEqual_FloatFloat |
| 185 (0x00b9) | 27 | Core.Object.SubtractEqual_FloatFloat |
| 186 (0x00ba) | 18 | Core.Object.Abs |
| 187 (0x00bb) | 5 | Core.Object.Sin |
| 189 (0x00bd) | 1 | Core.Object.Tan |
| 193 (0x00c1) | 4 | Core.Object.Sqrt |
| 195 (0x00c3) | 16 | Core.Object.FRand |
| 196 (0x00c4) | 3 | Core.Object.GreaterGreaterGreater_IntInt |
| 211 (0x00d3) | 7 | Core.Object.Subtract_PreVector |
| 212 (0x00d4) | 81 | Core.Object.Multiply_VectorFloat |
| 213 (0x00d5) | 82 | Core.Object.Multiply_FloatVector |
| 214 (0x00d6) | 10 | Core.Object.Divide_VectorFloat |
| 215 (0x00d7) | 111 | Core.Object.Add_VectorVector |
| 216 (0x00d8) | 128 | Core.Object.Subtract_VectorVector |
| 217 (0x00d9) | 6 | Core.Object.EqualEqual_VectorVector |
| 218 (0x00da) | 19 | Core.Object.NotEqual_VectorVector |
| 219 (0x00db) | 32 | Core.Object.Dot_VectorVector |
| 220 (0x00dc) | 1 | Core.Object.Cross_VectorVector |
| 221 (0x00dd) | 10 | Core.Object.MultiplyEqual_VectorFloat |
| 223 (0x00df) | 23 | Core.Object.AddEqual_VectorVector |
| 224 (0x00e0) | 2 | Core.Object.SubtractEqual_VectorVector |
| 225 (0x00e1) | 45 | Core.Object.VSize |
| 226 (0x00e2) | 45 | Core.Object.Normal |
| 229 (0x00e5) | 19 | Core.Object.GetAxes |
| 231 (0x00e7) | 127 | Core.Object.Log |
| 234 (0x00ea) | 5 | Core.Object.Right |
| 235 (0x00eb) | 10 | Core.Object.Caps |
| 236 (0x00ec) | 46 | Core.Object.Chr |
| 242 (0x00f2) | 19 | Core.Object.EqualEqual_BoolBool |
| 243 (0x00f3) | 7 | Core.Object.NotEqual_BoolBool |
| 244 (0x00f4) | 31 | Core.Object.FMin |
| 245 (0x00f5) | 22 | Core.Object.FMax |
| 246 (0x00f6) | 21 | Core.Object.FClamp |
| 249 (0x00f9) | 5 | Core.Object.Min |
| 250 (0x00fa) | 5 | Core.Object.Max |
| 251 (0x00fb) | 7 | Core.Object.Clamp |
| 252 (0x00fc) | 7 | Core.Object.VRand |
| 254 (0x00fe) | 124 | Core.Object.EqualEqual_NameName |
| 255 (0x00ff) | 77 | Core.Object.NotEqual_NameName |
| 256 (0x0100) | 59 | Engine.Actor.Sleep |
| 257 (0x0101) | 1 | Engine.Actor.BonePos |
| 258 (0x0102) | 22 | Core.Object.ClassIsChildOf |
| 259 (0x0103) | 55 | Engine.Actor.PlayAnim |
| 260 (0x0104) | 53 | Engine.Actor.LoopAnim |
| 261 (0x0105) | 28 | Engine.Actor.FinishAnim |
| 262 (0x0106) | 34 | Engine.Actor.SetCollision |
| 263 (0x0107) | 4 | Engine.Actor.HasAnim |
| 264 (0x0108) | 96 | Engine.Actor.PlaySound |
| 265 (0x0109) | 4 | Engine.Actor.CreateAnimChannel |
| 266 (0x010a) | 3 | Engine.Actor.Move |
| 267 (0x010b) | 41 | Engine.Actor.SetLocation |
| 268 (0x010c) | 3 | Engine.Actor.BoneNumber |
| 269 (0x010d) | 1 | Engine.Actor.BoneName |
| 272 (0x0110) | 19 | Engine.Actor.SetOwner |
| 274 (0x0112) | 1 | Engine.Actor.GetRenderExtent |
| 276 (0x0114) | 23 | Core.Object.GreaterGreater_VectorRotator |
| 277 (0x0115) | 14 | Engine.Actor.Trace |
| 278 (0x0116) | 67 | Engine.Actor.Spawn |
| 279 (0x0117) | 63 | Engine.Actor.Destroy |
| 280 (0x0118) | 23 | Engine.Actor.SetTimer |
| 281 (0x0119) | 33 | Core.Object.IsInState |
| 282 (0x011a) | 11 | Engine.Actor.IsAnimating |
| 283 (0x011b) | 2 | Engine.Actor.SetCollisionSize |
| 284 (0x011c) | 13 | Core.Object.GetStateName |
| 285 (0x011d) | 3 | Engine.Actor.TraceTexture |
| 287 (0x011f) | 3 | Core.Object.Multiply_RotatorFloat |
| 293 (0x0125) | 17 | Engine.Actor.GetAnimGroup |
| 294 (0x0126) | 1 | Engine.Actor.TweenAnim |
| 296 (0x0128) | 9 | Core.Object.Multiply_VectorVector |
| 297 (0x0129) | 6 | Core.Object.MultiplyEqual_VectorVector |
| 298 (0x012a) | 8 | Engine.Actor.SetBase |
| 299 (0x012b) | 30 | Engine.Actor.SetRotation |
| 300 (0x012c) | 1 | Core.Object.MirrorVectorByNormal |
| 301 (0x012d) | 14 | Engine.Actor.FinishInterpolation |
| 303 (0x012f) | 89 | Core.Object.IsA |
| 304 (0x0130) | 113 | Engine.Actor.AllActors |
| 309 (0x0135) | 2 | Engine.Actor.TraceActors |
| 310 (0x0136) | 1 | Engine.Actor.RadiusActors |
| 311 (0x0137) | 1 | Engine.Actor.VisibleActors |
| 312 (0x0138) | 1 | Engine.Actor.VisibleCollidingActors |
| 316 (0x013c) | 3 | Core.Object.Add_RotatorRotator |
| 317 (0x013d) | 9 | Core.Object.Subtract_RotatorRotator |
| 318 (0x013e) | 6 | Core.Object.AddEqual_RotatorRotator |
| 329 (0x0149) | 2 | Engine.Actor.IsSoftwareRendering |
| 465 (0x01d1) | 1 | Engine.Canvas.DrawText |
| 466 (0x01d2) | 1 | Engine.Canvas.DrawTile |
| 467 (0x01d3) | 1 | Engine.Canvas.DrawActor |
| 470 (0x01d6) | 1 | Engine.Canvas.TextSize |
| 500 (0x01f4) | 6 | Engine.Pawn.MoveTo |
| 502 (0x01f6) | 1 | Engine.Pawn.MoveToward |
| 508 (0x01fc) | 7 | Engine.Pawn.TurnTo |
| 510 (0x01fe) | 14 | Engine.Pawn.TurnToward |
| 512 (0x0200) | 11 | Engine.Actor.MakeNoise |
| 514 (0x0202) | 2 | Engine.Pawn.LineOfSightTo |
| 518 (0x0206) | 1 | Engine.Pawn.FindPathTo |
| 520 (0x0208) | 2 | Engine.Pawn.actorReachable |
| 524 (0x020c) | 1 | Engine.Pawn.FindStairRotation |
| 529 (0x0211) | 1 | Engine.Pawn.AddPawn |
| 530 (0x0212) | 1 | Engine.Pawn.RemovePawn |
| 531 (0x0213) | 1 | Engine.Pawn.PickTarget |
| 534 (0x0216) | 1 | Engine.Pawn.PickAnyTarget |
| 536 (0x0218) | 19 | Core.Object.SaveConfig |
| 546 (0x0222) | 2 | Engine.PlayerPawn.UpdateURL |
| 548 (0x0224) | 1 | Engine.Actor.FastTrace |
| 553 (0x0229) | 1 | Engine.Pawn.FindPath |
| 568 (0x0238) | 13 | Engine.Actor.StopSound |
| 1033 (0x0409) | 55 | Core.Object.RandRange |
| 3969 (0x0f81) | 12 | Engine.Actor.MoveSmooth |
| 3970 (0x0f82) | 58 | Engine.Actor.SetPhysics |
| 3971 (0x0f83) | 3 | Engine.Actor.AutonomousPhysics |

## Construct totals

- latent calls (named): 1
- latent calls (native slots): 124
- GotoState calls: 0
- GotoState calls (native slots): 195
- GotoLabel ops: 17
- states with code: 130
- state-local function streams: 367
- label tables: 75
- label entries: 96
- foreach (EX_Iterator): 118
- EX_ArrayElement: 293
- EX_DynArrayElement: 0
- VectorConst: 141
- RotationConst: 26
- instance/default-var writes: 1127
- StructMember: 1061
- Context/ClassContext: 3415
- New: 0

## Aborting streams

(none)

## Totals

- streams walked: 1952
- streams aborted: 0
- state streams: 130
- bytes covered: 162617
- opcodes decoded: 68479
