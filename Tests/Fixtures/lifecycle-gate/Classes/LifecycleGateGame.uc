class LifecycleGateGame extends GameInfo;

var LifecycleGateProbe Probe;

event InitGame(string Options, out string Error)
{
	Super.InitGame(Options, Error);
	if (Error != "")
		return;

	Probe = Spawn(class'LifecycleGateProbe', None, 'LifecycleGateProbe');
	Probe.StableKey = 'ProbeA';
	Probe = Spawn(class'LifecycleGateProbe', None, 'LifecycleGateProbe');
	Probe.StableKey = 'ProbeB';
	Spawn(class'LifecycleGateActor', None, 'LifecycleGate');
}
