//===============================================================================
//  #6 Glanmore Peakes
//===============================================================================

class  WCPeakes extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardPeakesTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Peakessmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardPeakesBigTexture FILE=TEXTURES\menu\Folio\Cards\Peakesbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Glanmore Peakes";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=6
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE090"
    skin(0)=Texture'HProps.skins.WizardCardPeakesTex0'
	textureBig=Texture'WizCardPeakesBigTexture'
	strDescriptionId="WizCard_0044"
}
