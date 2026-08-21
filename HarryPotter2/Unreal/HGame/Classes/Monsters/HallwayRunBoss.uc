class HallwayRunBoss expands baseBoss;

//*************************************************************************************************************************
event Trigger( Actor Other, Pawn EventInstigator )
{
	playerHarry.StopBossEncounter();
}
